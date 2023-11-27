use clap::Parser;

use hex;
use chia_protocol::FullBlock;
use chia_traits::Streamable;

use sqlite::{State, Connection};

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    #[arg(short, long)]
    file: String,
}

fn print_block_foliage(conn: &Connection, height: u32) {

    let mut stmt = conn
        .prepare("SELECT block FROM full_blocks WHERE in_main_chain=0 AND height=?")
        .expect("failed to prepare SQL statement pulling in main blocks");

    stmt.bind((1, height as i64)).expect("failed to bind block height to SQLite query");

    while let Ok(State::Row) = stmt.next() {
        let block_buffer = stmt.read::<Vec<u8>, _>(0).expect("invalid block blob");

        let block_buffer =
            zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(block_buffer))
                .expect("failed to decompress block");
        let block =
            FullBlock::from_bytes_unchecked(&block_buffer).expect("failed to parse FullBlock");

        println!("block {:9d} -    foliage: {}", block.height(), hex::encode(block.foliage.hash()));
    }
}

fn main() {
    let args = Args::parse();

    let connection = sqlite::open(args.file).expect("failed to open database file");

    let mut orphans = connection
        .prepare("SELECT height, block FROM full_blocks WHERE in_main_chain=0 ORDER BY height")
        .expect("failed to prepare SQL statement enumerating orphaned blocks");

    let mut last_height: i64 = -1;
    while let Ok(State::Row) = orphans.next() {
        let height: u32 = orphans
            .read::<i64, _>(0)
            .expect("missing height")
            .try_into()
            .expect("invalid height in block record");
        let block_buffer = orphans.read::<Vec<u8>, _>(1).expect("invalid block blob");

        let block_buffer =
            zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(block_buffer))
                .expect("failed to decompress block");
        let block =
            FullBlock::from_bytes_unchecked(&block_buffer).expect("failed to parse FullBlock");

        if last_height != height.into() {
            last_height = height.into();
            print_block_foliage(&connection, height.into());
        }

        println!("   orphan {:9d} - foliage: {}", block.height(), hex::encode(block.foliage.hash()));
    }
}

