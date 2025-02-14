use clap::Parser;
use rusqlite::Connection;

/// Prints information about a blockchain peak and can roll it back to a
/// specified height
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file
    file: String,

    /// Set the specified height to the new peak. This will update:
    ///   * the coin store
    ///   * the peak height
    ///   * the blocks past the peak (they will be marked as not part of the
    ///     chain)
    #[arg(long)]
    rollback: Option<u32>,
}

fn main() {
    let args = Args::parse();

    println!("opening blockchain database file: {}", args.file);

    let mut connection = Connection::open(&args.file).expect("failed to open database file");

    let mut select_peak = connection
        .prepare("SELECT hash FROM current_peak WHERE key=0;")
        .expect("failed to prepare SQL statement finding peak");

    let mut select_block_by_hash = connection
        .prepare("SELECT height FROM full_blocks WHERE header_hash=?;")
        .expect("failed to prepare SQL statement finding blocks by hash");

    let peak_hash = select_peak
        .query([])
        .expect("failed to query current peak")
        .next()
        .expect("missing peak")
        .expect("missing peak")
        .get::<_, [u8; 32]>(0)
        .expect("invalid peak hash");

    let peak_height = select_block_by_hash
        .query([peak_hash])
        .expect("failed to query block")
        .next()
        .expect("missing peak block")
        .expect("missing peak block")
        .get::<_, u32>(0)
        .expect("invalid peak height");

    println!("peak height: {peak_height}");
    println!("peak hash: {}", hex::encode(peak_hash));

    drop(select_peak);
    drop(select_block_by_hash);

    if let Some(rollback) = args.rollback {
        println!("Rolling back to height {rollback}");
        if rollback > peak_height {
            println!("new height is greater than peak, leaving database untouched");
            return;
        }
        let tx = connection
            .transaction()
            .expect("failed to start transaction");

        let mut select_block_by_height = tx
            .prepare("SELECT header_hash FROM full_blocks WHERE height=? AND in_main_chain=1;")
            .expect("failed to prepare SQL statement finding blocks by height");

        let new_peak_hash = select_block_by_height
            .query([rollback])
            .expect("block missing at height")
            .next()
            .expect("missing block")
            .expect("missing block")
            .get::<_, [u8; 32]>(0)
            .expect("invalid header hash");
        drop(select_block_by_height);

        println!("updating new peak to {}", hex::encode(peak_hash));
        tx.execute(
            "UPDATE current_peak SET hash=? WHERE key = 0;",
            [new_peak_hash],
        )
        .expect("setting peak hash");
        println!("clearing in_main_chain for all blocks with height above {rollback}");
        tx.execute(
            "UPDATE full_blocks SET in_main_chain=0 WHERE height > ?;",
            [rollback],
        )
        .expect("updating in_main_chain");
        println!("deleting all coins created after height {rollback}");
        tx.execute(
            "DELETE FROM coin_record WHERE confirmed_index>?",
            [rollback],
        )
        .expect("remove rolled-back coins");
        println!("unspending all coins spent after height {rollback}");
        tx.execute(
            "UPDATE coin_record SET spent_index=0 WHERE spent_index>?",
            [rollback],
        )
        .expect("unspend rolled-back coins");

        tx.commit().expect("failed to commit updates");
    }
}
