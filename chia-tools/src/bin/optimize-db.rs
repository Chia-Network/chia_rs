use clap::Parser;
use rusqlite::Connection;

/// Optimize a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to optimize
    file: String,

    /// Path to destination blockchain database file
    output: String,
}

fn main() {
    let args = Args::parse();

    let connection = Connection::open(args.file).expect("failed to open database file");
    let peak_hash: [u8; 32] = connection
        .prepare("SELECT hash FROM current_peak WHERE key = 0")
        .expect("prepare")
        .query([])
        .expect("query")
        .next()
        .expect("current_peak")
        .expect("current_peak")
        .get(0)
        .expect("get current_peak");

    println!("peak block hash: {}", hex::encode(peak_hash));

    let peak_height: u32 = connection
        .prepare("SELECT height FROM full_blocks WHERE header_hash=?")
        .expect("prepare")
        .query([peak_hash])
        .expect("query")
        .next()
        .expect("peak height")
        .expect("peak height")
        .get(0)
        .expect("get peak height");

    println!("peak height: {peak_height}");

    println!("dropping orphaned blocks...");
    let removed = connection
        .execute(
            "DELETE FROM full_blocks WHERE in_main_chain=0 AND height < ?",
            [peak_height - 10],
        )
        .expect("drop orphaned blocks");

    println!("removed {removed} orphaned blocks");

    let compact: usize = connection
        .prepare(
            "SELECT COUNT(*) FROM full_blocks WHERE is_fully_compactified=1 AND in_main_chain=1",
        )
        .expect("prepare compact")
        .query([])
        .expect("query compact")
        .next()
        .expect("compact")
        .expect("compact")
        .get(0)
        .expect("compact");

    println!(
        "{} fully compact blocks ({:.2}%)",
        compact,
        compact as f64 / peak_height as f64 * 100.0
    );

    println!("analyze block table");
    let _ = connection
        .execute("ANALYZE full_blocks", [])
        .expect("analyze");

    println!("analyze coins table");
    let _ = connection
        .execute("ANALYZE coin_record", [])
        .expect("analyze");

    println!("analyze hints table");
    let _ = connection.execute("ANALYZE hints", []).expect("analyze");

    println!("vacuum into {}", args.output);
    let _ = connection
        .execute("VACUUM INTO ?", [args.output])
        .expect("vacuum");
    println!("all done!");
}
