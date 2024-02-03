use chia_protocol::FullBlock;
use chia_traits::streamable::Streamable;
use clap::Parser;
use rusqlite::Connection;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file
    file: String,

    /// The header hash of the block whose generator to pull out
    header_hash: String,
}

fn main() {
    let args = Args::parse();

    let connection = Connection::open(args.file).expect("failed to open database file");

    let hash: [u8; 32] = hex::decode(args.header_hash)
        .expect("header-hash must be hex")
        .try_into()
        .expect("header-hash must be 32 bytes");

    let mut hash_lookup = connection
        .prepare("SELECT block FROM full_blocks WHERE header_hash=? AND in_main_chain=1")
        .expect("failed to prepare SQL statement");

    let mut rows = hash_lookup.query([hash]).expect("failed to query blocks");

    while let Ok(Some(row)) = rows.next() {
        let buffer: Vec<u8> = row.get(0).expect("invalid block blob");

        let block = zstd::stream::decode_all(&mut std::io::Cursor::new(buffer))
            .expect("failed to decompress block");
        let block = FullBlock::from_bytes_unchecked(&block).expect("failed to parse FullBlock");

        let program = block
            .transactions_generator
            .expect("not a transaction block");

        println!("{}\n", hex::encode(program.as_ref()))
    }
}
