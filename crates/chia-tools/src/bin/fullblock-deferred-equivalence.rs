#[path = "fullblock-deferred-equivalence/legacy_full_block.rs"]
mod legacy_full_block;

use chia_protocol::{FullBlock, Program};
use chia_traits::Streamable;
use clap::Parser;
use legacy_full_block::LegacyFullBlock;
use rusqlite::{Connection, params};
use std::io::{Cursor, Write};
use std::time::{Duration, Instant};

/// Compare eager FullBlock parsing against deferred generator parsing.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to scan
    file: String,

    /// Start scanning at this block height
    #[arg(long, default_value_t = 0)]
    start_height: u32,

    /// Stop scanning after this block height
    #[arg(long)]
    max_height: Option<u32>,

    /// Stop at the first parse, comparison, or serialization mismatch
    #[arg(long, default_value_t = false)]
    fail_fast: bool,

    /// Seconds between progress updates
    #[arg(long, default_value_t = 2)]
    progress_interval_secs: u64,
}

fn short_hex(bytes: &[u8]) -> String {
    const PREFIX_LEN: usize = 32;
    let prefix_len = bytes.len().min(PREFIX_LEN);
    let suffix = if bytes.len() > PREFIX_LEN { "..." } else { "" };
    format!("{}{}", hex::encode(&bytes[..prefix_len]), suffix)
}

fn record_mismatch(height: u32, category: &str, detail: impl std::fmt::Display) {
    println!("mismatch at height {height}: {category}: {detail}");
}

fn compare_value<T: PartialEq + std::fmt::Debug>(
    height: u32,
    category: &str,
    eager: T,
    deferred: T,
) -> bool {
    if eager == deferred {
        true
    } else {
        record_mismatch(
            height,
            category,
            format_args!("LegacyFullBlock={eager:?}, FullBlock={deferred:?}"),
        );
        false
    }
}

fn compare_generator(height: u32, eager: &Option<Program>, deferred: &Option<Program>) -> bool {
    match (eager, deferred) {
        (None, None) => true,
        (Some(eager), Some(deferred)) if eager.as_slice() == deferred.as_slice() => true,
        (Some(eager), Some(deferred)) => {
            record_mismatch(
                height,
                "transactions_generator",
                format_args!(
                    "bytes differ: LegacyFullBlock len={} prefix={}, FullBlock len={} prefix={}",
                    eager.as_slice().len(),
                    short_hex(eager.as_slice()),
                    deferred.as_slice().len(),
                    short_hex(deferred.as_slice())
                ),
            );
            false
        }
        (None, Some(deferred)) => {
            record_mismatch(
                height,
                "transactions_generator",
                format_args!(
                    "LegacyFullBlock=None, FullBlock=Some(len={}, prefix={})",
                    deferred.as_slice().len(),
                    short_hex(deferred.as_slice())
                ),
            );
            false
        }
        (Some(eager), None) => {
            record_mismatch(
                height,
                "transactions_generator",
                format_args!(
                    "LegacyFullBlock=Some(len={}, prefix={}), FullBlock=None",
                    eager.as_slice().len(),
                    short_hex(eager.as_slice())
                ),
            );
            false
        }
    }
}

fn compare_serialized(height: u32, original: &[u8], eager: &[u8], deferred: &[u8]) -> bool {
    let mut ok = true;
    if eager != original {
        record_mismatch(
            height,
            "LegacyFullBlock serialization",
            format_args!(
                "does not match original: original len={} prefix={}, serialized len={} prefix={}",
                original.len(),
                short_hex(original),
                eager.len(),
                short_hex(eager)
            ),
        );
        ok = false;
    }
    if deferred != original {
        record_mismatch(
            height,
            "FullBlock serialization",
            format_args!(
                "does not match original: original len={} prefix={}, serialized len={} prefix={}",
                original.len(),
                short_hex(original),
                deferred.len(),
                short_hex(deferred)
            ),
        );
        ok = false;
    }
    if eager != deferred {
        record_mismatch(
            height,
            "serialization equality",
            format_args!(
                "LegacyFullBlock len={} prefix={}, FullBlock len={} prefix={}",
                eager.len(),
                short_hex(eager),
                deferred.len(),
                short_hex(deferred)
            ),
        );
        ok = false;
    }
    ok
}

fn compare_blocks(height: u32, block_bytes: &[u8]) -> bool {
    let eager = match LegacyFullBlock::from_bytes_unchecked(block_bytes) {
        Ok(block) => block,
        Err(error) => {
            record_mismatch(height, "LegacyFullBlock parse", error);
            return false;
        }
    };
    let deferred = match FullBlock::from_bytes_unchecked(block_bytes) {
        Ok(block) => block,
        Err(error) => {
            record_mismatch(height, "FullBlock parse", error);
            return false;
        }
    };

    let mut ok = true;
    ok &= compare_value(height, "height", eager.height(), deferred.height());
    ok &= compare_value(height, "weight", eager.weight(), deferred.weight());
    ok &= compare_value(
        height,
        "total_iters",
        eager.total_iters(),
        deferred.total_iters(),
    );
    ok &= compare_value(
        height,
        "prev_header_hash",
        eager.prev_header_hash(),
        deferred.prev_header_hash(),
    );
    ok &= compare_value(
        height,
        "header_hash",
        eager.header_hash(),
        deferred.header_hash(),
    );
    ok &= compare_value(
        height,
        "is_transaction_block",
        eager.is_transaction_block(),
        deferred.is_transaction_block(),
    );
    ok &= compare_value(
        height,
        "is_fully_compactified",
        eager.is_fully_compactified(),
        deferred.is_fully_compactified(),
    );

    let deferred_generator = match deferred.transactions_generator() {
        Ok(generator) => generator,
        Err(error) => {
            record_mismatch(height, "FullBlock transactions_generator", error);
            return false;
        }
    };
    ok &= compare_generator(height, eager.transactions_generator(), &deferred_generator);

    let deferred_ref_list = match deferred.transactions_generator_ref_list() {
        Ok(ref_list) => ref_list,
        Err(error) => {
            record_mismatch(height, "FullBlock transactions_generator_ref_list", error);
            return false;
        }
    };
    ok &= compare_value(
        height,
        "transactions_generator_ref_list",
        eager.transactions_generator_ref_list().clone(),
        deferred_ref_list,
    );

    let eager_bytes = match eager.to_bytes() {
        Ok(bytes) => bytes,
        Err(error) => {
            record_mismatch(height, "LegacyFullBlock serialization", error);
            return false;
        }
    };
    let deferred_bytes = match deferred.to_bytes() {
        Ok(bytes) => bytes,
        Err(error) => {
            record_mismatch(height, "FullBlock serialization", error);
            return false;
        }
    };
    ok &= compare_serialized(height, block_bytes, &eager_bytes, &deferred_bytes);

    ok
}

fn main() {
    let args = Args::parse();
    println!("opening blockchain database file: {}", args.file);

    let connection = Connection::open(&args.file).expect("failed to open database file");
    let mut statement = connection
        .prepare(
            "SELECT height, block \
             FROM full_blocks \
             WHERE in_main_chain=1 AND height >= ? AND (? IS NULL OR height <= ?) \
             ORDER BY height",
        )
        .expect("failed to prepare SQL statement enumerating blocks");

    let mut rows = statement
        .query(params![args.start_height, args.max_height, args.max_height])
        .expect("failed to query blocks");

    let mut checked = 0_u64;
    let mut errors = 0_u64;
    let mut last_height = args.start_height;
    let mut last_checked = 0_u64;
    let mut last_progress = Instant::now();
    let progress_interval = Duration::from_secs(args.progress_interval_secs);

    while let Ok(Some(row)) = rows.next() {
        let height = row.get::<_, u32>(0).expect("missing height");
        let compressed_block = row.get::<_, Vec<u8>>(1).expect("invalid block blob");
        let block_bytes = zstd::stream::decode_all(&mut Cursor::new(compressed_block))
            .expect("failed to decompress block");

        checked += 1;
        if !compare_blocks(height, &block_bytes) {
            errors += 1;
            if args.fail_fast {
                break;
            }
        }
        last_height = height;

        if last_progress.elapsed() >= progress_interval {
            let rate = (checked - last_checked) as f64 / last_progress.elapsed().as_secs_f64();
            print!(
                "\rheight: {height} checked: {checked} errors: {errors} ({rate:0.1} blocks/s)   "
            );
            let _ = std::io::stdout().flush();
            last_checked = checked;
            last_progress = Instant::now();
        }
    }

    println!(
        "\nfinished deferred FullBlock equivalence scan: checked={checked}, errors={errors}, last_height={last_height}"
    );
    if errors != 0 {
        std::process::exit(1);
    }
}
