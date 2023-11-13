// This is a throw-away tool to scan a chia blockchain database for spends that
// look like they might support "fast-forward" (or rebasing on top of a new
// coin). It was used to pull out real-life examples of spends that would
// satisfy the requirements on the fast-foward feature.

use chia_protocol::Bytes;
use clap::Parser;

use chia_protocol::bytes::Bytes32;
use chia_protocol::{coin::Coin, coin_spend::CoinSpend, program::Program};
use chia_tools::{iterate_tx_blocks, visit_spends};
use chia_traits::streamable::Streamable;
use chia_wallet::singleton::SINGLETON_TOP_LAYER_PUZZLE_HASH;
use clvm_traits::FromPtr;
use clvm_utils::{tree_hash, CurriedProgram};
use clvmr::allocator::NodePtr;
use clvmr::serde::node_to_bytes;
use clvmr::Allocator;
use std::thread::available_parallelism;
use threadpool::ThreadPool;

/// Analyze the spends in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    file: String,

    /// The number of threads to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// stop running block generators when reaching this height
    #[arg(short, long)]
    max_height: Option<u32>,

    /// start running block generators at this height
    #[arg(long, default_value_t = 0)]
    start_height: u32,
}

fn main() {
    let args = Args::parse();

    let pool = ThreadPool::new(
        args.num_jobs
            .unwrap_or_else(|| available_parallelism().unwrap().into()),
    );

    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    let seen_puzzles = Arc::new(Mutex::new(HashSet::<Bytes32>::new()));
    let seen_singletons = Arc::new(Mutex::new(HashSet::<Bytes32>::new()));

    iterate_tx_blocks(
        &args.file,
        args.start_height,
        args.max_height,
        |height, block, block_refs| {
            // this is called for each transaction block
            let max_cost = block.transactions_info.unwrap().cost;
            let prg = block.transactions_generator.unwrap();

            let seen_puzzles = seen_puzzles.clone();
            let seen_singletons = seen_singletons.clone();
            pool.execute(move || {
                let mut a = Allocator::new_limited(500000000, 62500000, 62500000);

                let generator = prg.as_ref();

                visit_spends(
                    &mut a,
                    generator,
                    &block_refs,
                    max_cost,
                    |a, parent_coin_info, amount, puzzle, solution| {
                        let puzzle_hash = Bytes32::from(tree_hash(a, puzzle));
                        let mod_hash = match <CurriedProgram<NodePtr, NodePtr>>::from_ptr(a, puzzle)
                        {
                            Ok(uncurried) => Bytes32::from(tree_hash(a, uncurried.program)),
                            _ => puzzle_hash,
                        };

                        let run_puzzle = seen_puzzles.lock().unwrap().insert(mod_hash);
                        let fast_forward = (mod_hash == SINGLETON_TOP_LAYER_PUZZLE_HASH)
                            && seen_singletons.lock().unwrap().insert(puzzle_hash);

                        if !run_puzzle && !fast_forward {
                            return;
                        }
                        use std::fs::write;

                        let puzzle_bytes = node_to_bytes(a, puzzle).expect("puzzle reveal");
                        let puzzle_reveal = Program::new(Bytes::from(puzzle_bytes));

                        let solution_bytes = node_to_bytes(a, solution).expect("solution");
                        let solution = Program::new(Bytes::from(solution_bytes));

                        let coin = Coin {
                            parent_coin_info,
                            puzzle_hash,
                            amount,
                        };
                        let spend = CoinSpend {
                            coin,
                            puzzle_reveal,
                            solution,
                        };

                        let mut bytes = Vec::<u8>::new();
                        spend.stream(&mut bytes).expect("stream CoinSpend");
                        if run_puzzle {
                            let directory = "../fuzz/corpus/run-puzzle";
                            let _ = std::fs::create_dir_all(directory);
                            write(format!("{directory}/{mod_hash}.spend"), &bytes).expect("write");
                            println!("{height}: {mod_hash}");
                        }

                        if fast_forward {
                            let directory = "../fuzz/corpus/fast-forward";
                            let _ = std::fs::create_dir_all(directory);
                            write(format!("{directory}/{puzzle_hash}.spend"), bytes)
                                .expect("write");
                            println!("{height}: {puzzle_hash}");
                        }
                    },
                )
                .expect("failed to run block generator");
            });
        },
    );

    assert_eq!(pool.panic_count(), 0);

    pool.join();
}
