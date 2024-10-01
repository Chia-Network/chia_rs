use clap::Parser;

use std::io::Write;
use std::time::SystemTime;

use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::gen::flags::{ALLOW_BACKREFS, MEMPOOL_MODE};
use chia_consensus::gen::run_block_generator::{run_block_generator, run_block_generator2};
use chia_tools::iterate_tx_blocks;
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

    /// Start at this block height
    #[arg(short, long, default_value_t = 225694)]
    start: u32,

    /// The height to stop at
    #[arg(short, long, default_value_t = 0xffffffff)]
    end: u32,
}

fn main() {
    let args = Args::parse();

    let flags = if args.mempool_mode { MEMPOOL_MODE } else { 0 } | ALLOW_BACKREFS;

    let mut output =
        std::fs::File::create("chain-resource-usage.log").expect("failed to open output file");

    // We only create a single allocator and keep reusing it
    let mut a = Allocator::new_limited(500_000_000);
    let allocator_checkpoint = a.checkpoint();
    let mut prev_timestamp = 0;
    iterate_tx_blocks(
        &args.file,
        args.start,
        Some(args.end),
        |height, block, block_refs| {
            // after the hard fork, we run blocks without paying for the
            // CLVM generator ROM
            let block_runner = if height >= 5_496_000 {
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

            if prev_timestamp == 0 {
                prev_timestamp = ftb.timestamp;
            }
            let time_delta = ftb.timestamp - prev_timestamp;
            prev_timestamp = ftb.timestamp;

            a.restore_checkpoint(&allocator_checkpoint);

            let start_run_block = SystemTime::now();
            let conditions = block_runner(
                &mut a,
                generator,
                &block_refs,
                ti.cost,
                flags,
                &TEST_CONSTANTS,
            )
            .expect("failed to run block generator");

            let execute_timing = start_run_block
                .elapsed()
                .expect("failed to get system time");

            assert_eq!(conditions.cost, ti.cost);
            output
                .write_fmt(format_args!(
                    "{height} \
                atoms: {} \
                small_atoms: {} \
                pairs: {} \
                heap: {} \
                block_cost: {} \
                execute_time: {} \
                timestamp: {} \
                time_delta: {time_delta} \
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
        },
    );
}
