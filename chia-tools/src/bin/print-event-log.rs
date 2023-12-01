use chia_streamable_macro::Streamable;
use chia_traits::Streamable;
use clap::Parser;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read};
use zstd::stream::decode_all;

use chia_protocol::{Bytes, Bytes32, NewPeak, RespondSignagePoint, SpendBundle, UnfinishedBlock};

/*
Some interesting SQL queries on a database with UnfinishedBlocks:

Start and end time of the log:

   SELECT datetime(timestamp, 'unixepoch') FROM unfinished_blocks ORDER BY timestamp ASC LIMIT 1;
   SELECT datetime(timestamp, 'unixepoch') FROM unfinished_blocks ORDER BY timestamp DESC LIMIT 1;

List proofs-of-space that's been used to create more than one block:

   SELECT hex(proof_of_space), COUNT(*) FROM (SELECT * FROM unfinished_blocks GROUP BY proof_of_space,foliage_hash) GROUP BY proof_of_space HAVING COUNT(*) > 1;

List all blocks that used a specific proof-of-space:

   SELECT peer,DATETIME(timestamp, 'unixepoch'),hex(proof_of_space),hex(foliage_hash),hex(farmer_reward_address),hex(pool_reward_address) FROM unfinished_blocks WHERE proof_of_space==x'';

List blocks that had more than 1 valid proof of space:

   SELECT hex(prev_hash),COUNT(*) FROM (SELECT * FROM unfinished_blocks GROUP BY prev_hash,proof_of_space) GROUP BY prev_hash HAVING COUNT(*) > 1;

   SELECT peer,DATETIME(timestamp, 'unixepoch'),hex(proof_of_space),hex(foliage_hash),hex(farmer_reward_address),hex(pool_reward_address) FROM unfinished_blocks WHERE prev_hash==x'' GROUP BY proof_of_space,foliage_hash ORDER BY proof_of_space;

List the valid proofs of space at a specific height (prev_hash):

   SELECT peer,DATETIME(timestamp, 'unixepoch'),hex(proof_of_space),hex(foliage_hash),hex(farmer_reward_address),hex(pool_reward_address) FROM unfinished_blocks WHERE prev_hash==x'';

List information about a peer:

   SELECT * from peers WHERE rowid==<peer-id>;
*/

/// Print an event log printed by the full node of chia-blockchain
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the log file
    input: Vec<String>,

    /// Only print messages of the specified type. One of the following strings:
    /// "peer", "in-unfinished", "out-unfinished", "sp", "tx", "new-peak".
    #[arg(long)]
    only_msg: Option<String>,

    /// only show messages from the specified peer index. The peer index is
    /// established by the initial "peer" message
    #[arg(long)]
    only_peer: Option<u32>,

    /// While printing, also save printed entries to the specified sqlite
    /// database. If the database file already exists, new rows will be added to
    /// the existing DB.
    #[arg(long)]
    save_db: Option<String>,
}

// msg_body is LogHeader
const START: u16 = 0;
// msg_body is PeerConnected
const NEW_PEER_CONNECTION: u16 = 1;
// msg_body is UnfinishedBlock
const INCOMING_UNFINISHED_BLOCK: u16 = 2;
const OUTGOING_UNFINISHED_BLOCK: u16 = 3;
// msg_body is RespondSignagePoint
const INCOMING_SIGNAGE_POINT: u16 = 4;
// msg_type is SpendBundle
const INCOMING_TRANSACTION: u16 = 5;
// msg_type is NewPeak
const NEW_PEAK: u16 = 6;

#[derive(Streamable, Debug)]
pub struct LogHeader {
    // version of the structured log format
    pub version: u8,
    // unix time of startup
    pub start_time: u64,
}

#[derive(Streamable, Debug)]
pub struct PeerConnected {
    pub version: String,
    pub protocol_version: String,
    pub outbound: bool,
    pub port: u16,
    pub peer_node_id: Bytes32,
    pub host: String,
    pub connection_type: u8,
}

#[derive(Streamable, Debug)]
pub struct Message {
    // timestamp of event
    pub timestamp: u32,
    // the index of the peer involved in this event (0 if no peer was involved)
    pub peer: u32,
    // that type to expect in msg_body
    pub msg_type: u16,
    // Streamable encoded message whose type is determined by msg_type
    pub msg_body: Bytes,
}

struct Filter {
    only_msg: Option<u16>,
    only_peer: Option<u32>,
}

impl Filter {
    fn new(args: &Args) -> Self {
        let msg_type = args.only_msg.as_ref().map(|m| match m.as_str() {
            "peer" => NEW_PEER_CONNECTION,
            "in-unfinished" => INCOMING_UNFINISHED_BLOCK,
            "out-unfinished" => OUTGOING_UNFINISHED_BLOCK,
            "sp" => INCOMING_SIGNAGE_POINT,
            "tx" => INCOMING_TRANSACTION,
            "new-peak" => NEW_PEAK,
            _ => panic!("unknown message type"),
        });

        Filter {
            only_msg: msg_type,
            only_peer: args.only_peer,
        }
    }
}

fn setup_db(name: &String) -> Connection {
    let connection = Connection::open(name).expect("failed to open database file");

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS peers(
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
            version BLOB,
            protocol_version BLOB,
            outbound TINYINT,
            port INT,
            peer_node_id BLOB,
            host BLOB,
            connection_type TINYINT,
            connection_time INT
        )",
            [],
        )
        .expect("failed to create peers table");

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS unfinished_blocks(
            peer INT,
            timestamp INT,
            prev_hash BLOB,
            proof_of_space BLOB,
            foliage_hash BLOB,
            farmer_reward_address BLOB,
            pool_reward_address BLOB,
            unfinished_block BLOB
        )",
            [],
        )
        .expect("failed to create UnfinishedBlocks table");

    connection
}

#[derive(Default)]
struct ProcessingState {
    start_time: u64,

    // maps the connection id of the log file we're reading to the connection ID
    // of the database we're writing to
    peer_map: HashMap<u32, i64>,
}

fn save_to_db(db: &Connection, state: &mut ProcessingState, filter: &Filter, msg: &Message) {
    // We don't apply the filter to the start message, as that's how we record
    // the start timestamp. Other timestamps are relative to this
    if msg.msg_type != START {
        if let Some(only_msg) = filter.only_msg {
            if msg.msg_type != only_msg {
                return;
            }
        }

        if let Some(peer) = filter.only_peer {
            if msg.peer != peer {
                return;
            }
        }
    }

    let body = decode_all(&mut Cursor::new(msg.msg_body.as_slice())).expect("zstd decompress");

    match msg.msg_type {
        START => {
            let log_header =
                LogHeader::from_bytes_unchecked(body.as_slice()).expect("parse LogHeader");
            state.start_time = log_header.start_time;
        }
        NEW_PEER_CONNECTION => {
            let peer =
                PeerConnected::from_bytes_unchecked(body.as_slice()).expect("parse PeerConnected");
            db.execute(
                "INSERT INTO peers (
                    version,
                    protocol_version,
                    outbound,
                    port,
                    peer_node_id,
                    host,
                    connection_type,
                    connection_time
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    peer.version,
                    peer.protocol_version,
                    peer.outbound,
                    peer.port,
                    peer.peer_node_id.as_slice(),
                    peer.host,
                    peer.connection_type,
                    state.start_time + msg.timestamp as u64
                ],
            )
            .expect("failed to insert into peer table");
            state.peer_map.insert(msg.peer, db.last_insert_rowid());
        }
        INCOMING_UNFINISHED_BLOCK => {
            let ub = UnfinishedBlock::from_bytes_unchecked(body.as_slice())
                .expect("parse UnfinishedBlock");

            db.execute(
                "INSERT INTO unfinished_blocks(
                    peer,
                    timestamp,
                    prev_hash,
                    proof_of_space,
                    foliage_hash,
                    farmer_reward_address,
                    pool_reward_address,
                    unfinished_block
                    ) VALUES(?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    state.peer_map.get(&msg.peer).expect("unknown peer id"),
                    state.start_time + msg.timestamp as u64,
                    ub.foliage.prev_block_hash.as_slice(),
                    ub.reward_chain_block
                        .proof_of_space
                        .to_bytes()
                        .expect("failed to serialize proof-of-space"),
                    ub.foliage.hash().as_slice(),
                    ub.foliage
                        .foliage_block_data
                        .farmer_reward_puzzle_hash
                        .as_slice(),
                    ub.foliage
                        .foliage_block_data
                        .pool_target
                        .puzzle_hash
                        .as_slice(),
                    body.as_slice()
                ],
            )
            .expect("failed to insert into peer table");
        }
        _ => {}
    }
}

fn print_msg(state: &mut ProcessingState, filter: &Filter, msg: &Message) {
    if let Some(only_msg) = filter.only_msg {
        if msg.msg_type != only_msg {
            return;
        }
    }

    if let Some(peer) = filter.only_peer {
        if msg.peer != peer {
            return;
        }
    }

    print!("{:10} {:10} ", msg.timestamp, msg.peer);

    let body = decode_all(&mut Cursor::new(msg.msg_body.as_slice())).expect("zstd decompress");

    match msg.msg_type {
        START => {
            let log_header =
                LogHeader::from_bytes_unchecked(body.as_slice()).expect("parse LogHeader");
            state.start_time = log_header.start_time;
            println!("{:?}", log_header);
        }
        NEW_PEER_CONNECTION => {
            let peer =
                PeerConnected::from_bytes_unchecked(body.as_slice()).expect("parse PeerConnected");
            println!("{:?}", peer);
        }
        INCOMING_UNFINISHED_BLOCK => {
            let ub = UnfinishedBlock::from_bytes_unchecked(body.as_slice())
                .expect("parse UnfinishedBlock");
            println!("{:?}", ub);
        }
        OUTGOING_UNFINISHED_BLOCK => println!(
            "{:?}",
            UnfinishedBlock::from_bytes_unchecked(body.as_slice()).expect("parse UnfinishedBlock")
        ),
        INCOMING_SIGNAGE_POINT => println!(
            "{:?}",
            RespondSignagePoint::from_bytes_unchecked(body.as_slice())
                .expect("parse RespondSignagePoint")
        ),
        INCOMING_TRANSACTION => println!(
            "{:?}",
            SpendBundle::from_bytes_unchecked(body.as_slice()).expect("parse SpendBundle")
        ),
        NEW_PEAK => println!(
            "{:?}",
            NewPeak::from_bytes_unchecked(body.as_slice()).expect("parse NewPeak")
        ),
        _ => println!(" UNKNOWN MESSAGE TYPE {}", msg.msg_type),
    }
}

fn main() {
    let args = Args::parse();
    let filter = Filter::new(&args);

    let conn = args.save_db.as_ref().map(setup_db);

    let mut state = ProcessingState::default();

    let mut buf = vec![0_u8; 1 << 20];
    let mut valid_data: usize = 0;

    for filename in &args.input {
        let mut file = File::open(filename).expect("read file");
        while let Ok(size) = file.read(&mut buf.as_mut_slice()[valid_data..]) {
            valid_data += size;
            let mut cursor = Cursor::new(&buf.as_slice()[..valid_data]);
            let mut position = 0;
            while let Ok(msg) = Message::parse::<true>(&mut cursor) {
                if let Some(db) = &conn {
                    save_to_db(db, &mut state, &filter, &msg);
                } else {
                    print_msg(&mut state, &filter, &msg);
                }
                position = cursor.position();
            }
            let remainder = position as usize..valid_data;
            valid_data = remainder.len();
            buf.copy_within(remainder, 0);
            if size == 0 && valid_data == 0 {
                break;
            }
        }
    }
}
