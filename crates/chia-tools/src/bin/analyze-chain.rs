use clap::Parser;

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread::available_parallelism;
use std::time::Instant;

use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::{DONT_VALIDATE_SIGNATURE, MEMPOOL_MODE};
use chia_consensus::run_block_generator::{run_block_generator, run_block_generator2};
use chia_tools::iterate_blocks;
use clvmr::Allocator;

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    #[arg(short, long)]
    file: String,

    /// Specifies whether to run the blocks in the stricter mempool mode or not
    #[arg(short, long, default_value_t = false)]
    mempool_mode: bool,

    /// The number of parallell thread to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// Start at this block height
    #[arg(short, long, default_value_t = 225694)]
    start: u32,

    /// The height to stop at
    #[arg(short, long, default_value_t = 0xffffffff)]
    end: u32,
}

fn main() {
    let args = Args::parse();

    let flags = if args.mempool_mode { MEMPOOL_MODE } else { 0 } | DONT_VALIDATE_SIGNATURE;

    let num_cores = args
        .num_jobs
        .unwrap_or_else(|| available_parallelism().unwrap().into());

    let pool = blocking_threadpool::Builder::new()
        .num_threads(num_cores)
        .queue_len(num_cores * 2)
        .build();

    let output = Arc::new(Mutex::new(
        std::fs::File::create("chain-resource-usage.log").expect("failed to open output file"),
    ));

    let mut start_time = Instant::now();

    iterate_blocks(
        &args.file,
        args.start,
        Some(args.end),
        |height, block, block_refs| {
            if start_time.elapsed().as_secs() > 5 {
                start_time = Instant::now();
                print!("  {height}\r");
                io::stdout().flush().unwrap();
            }
            if block.transactions_generator.is_none() {
                return;
            }
            let output = output.clone();
            pool.execute(move || {
                // after the hard fork, we run blocks without paying for the
                // CLVM generator ROM
                let block_runner = if height >= TEST_CONSTANTS.hard_fork_height {
                    run_block_generator2
                } else {
                    run_block_generator
                };

                let generator = block
                    .transactions_generator
                    .as_ref()
                    .expect("transactions_generator");

                let ti = block.transactions_info.as_ref().expect("transactions_info");

                let ftb = block
                    .foliage_transaction_block
                    .expect("foliage_transaction_block");

                let mut a = Allocator::new_limited(500_000_000);

                let start_run_block = Instant::now();
                let conditions = block_runner(
                    &mut a,
                    generator,
                    &block_refs,
                    ti.cost,
                    flags,
                    &ti.aggregated_signature,
                    None,
                    &TEST_CONSTANTS,
                )
                .expect("failed to run block generator");

                let execute_timing = start_run_block.elapsed();

                assert_eq!(conditions.cost, ti.cost);
                output
                    .lock()
                    .unwrap()
                    .write_fmt(format_args!(
                        "{height} \
                    atoms: {} \
                    small_atoms: {} \
                    pairs: {} \
                    heap: {} \
                    block_cost: {} \
                    execute_time: {} \
                    timestamp: {} \
                    \n",
                        a.atom_count(),
                        a.small_atom_count(),
                        a.pair_count(),
                        a.heap_size(),
                        ti.cost,
                        execute_timing.as_micros(),
                        ftb.timestamp,
                    ))
                    .expect("failed to write to output file");
            });
        },
    );

    pool.join();
    assert_eq!(pool.panic_count(), 0);
}
