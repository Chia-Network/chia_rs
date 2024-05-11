// This is a throw-away tool to scan a chia blockchain database for spends that
// look like they might support "fast-forward" (or rebasing on top of a new
// coin). It was used to pull out real-life examples of spends that would
// satisfy the requirements on the fast-foward feature.

use clap::Parser;

use chia_tools::{iterate_tx_blocks, visit_spends};
use chia_traits::streamable::Streamable;

use chia_protocol::{Bytes32, Coin, CoinSpend, Program, SpendBundle};
use chia_puzzles::singleton::SINGLETON_TOP_LAYER_PUZZLE_HASH;
use clvm_traits::{FromClvm, FromNodePtr};
use clvm_utils::{tree_hash, CurriedProgram};
use clvmr::allocator::NodePtr;
use clvmr::Allocator;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

/// Analyze the spends in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    file: String,

    /// The number of threads to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// generate spend-bundles
    #[arg(long, default_value_t = false)]
    spend_bundles: bool,

    /// stop running block generators when reaching this height
    #[arg(short, long)]
    max_height: Option<u32>,

    /// start running block generators at this height
    #[arg(long, default_value_t = 0)]
    start_height: u32,
}

fn main() {
    let args = Args::parse();

    let num_cores = args
        .num_jobs
        .unwrap_or_else(|| available_parallelism().unwrap().into());
    let pool = blocking_threadpool::Builder::new()
        .num_threads(num_cores)
        .queue_len(num_cores + 5)
        .build();

    use chia_bls::G2Element;
    use std::collections::HashSet;
    use std::fs::write;
    use std::sync::{Arc, Mutex};

    let seen_puzzles = Arc::new(Mutex::new(HashSet::<Bytes32>::new()));
    let seen_singletons = Arc::new(Mutex::new(HashSet::<Bytes32>::new()));

    let mut last_height = 0;
    let mut last_time = Instant::now();
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
                let mut a = Allocator::new_limited(500000000);

                let generator = prg.as_ref();

                let mut bundle = SpendBundle::new(vec![], G2Element::default());

                visit_spends(
                    &mut a,
                    generator,
                    &block_refs,
                    max_cost,
                    |a, parent_coin_info, amount, puzzle, solution| {
                        let puzzle_hash = Bytes32::from(tree_hash(a, puzzle));
                        let mod_hash =
                            match CurriedProgram::<NodePtr, NodePtr>::from_clvm(a, puzzle) {
                                Ok(uncurried) => Bytes32::from(tree_hash(a, uncurried.program)),
                                _ => puzzle_hash,
                            };

                        let run_puzzle = seen_puzzles.lock().unwrap().insert(mod_hash);
                        let fast_forward = (mod_hash == SINGLETON_TOP_LAYER_PUZZLE_HASH.into())
                            && seen_singletons.lock().unwrap().insert(puzzle_hash);

                        if !run_puzzle && !fast_forward && !args.spend_bundles {
                            return;
                        }
                        let puzzle_reveal =
                            Program::from_node_ptr(a, puzzle).expect("puzzle reveal");
                        let solution = Program::from_node_ptr(a, solution).expect("solution");
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

                        if args.spend_bundles {
                            bundle.coin_spends.push(spend.clone());
                        }

                        if !run_puzzle && !fast_forward {
                            return;
                        }
                        let mut bytes = Vec::<u8>::new();
                        spend.stream(&mut bytes).expect("stream CoinSpend");
                        if run_puzzle {
                            let directory = "../chia-consensus/fuzz/corpus/run-puzzle";
                            let _ = std::fs::create_dir_all(directory);
                            write(format!("{directory}/{mod_hash}.spend"), &bytes).expect("write");
                            println!("{height}: {mod_hash}");
                        }

                        if fast_forward {
                            let directory = "../chia-consensus/fuzz/corpus/fast-forward";
                            let _ = std::fs::create_dir_all(directory);
                            write(format!("{directory}/{puzzle_hash}.spend"), bytes)
                                .expect("write");
                            println!("{height}: {puzzle_hash}");
                        }
                    },
                )
                .expect("failed to run block generator");

                if args.spend_bundles {
                    let directory = "../chia-protocol/fuzz/corpus/spend-bundle";
                    let _ = std::fs::create_dir_all(directory);
                    let bytes = bundle.to_bytes().expect("to_bytes");
                    write(format!("{directory}/{height}.bundle"), bytes).expect("write");
                }
            });
            if last_time.elapsed() > Duration::new(4, 0) {
                let rate = (height - last_height) as f64 / last_time.elapsed().as_secs_f64();
                use std::io::Write;
                print!("\rheight: {height} ({rate:0.1} blocks/s)   ");
                let _ = std::io::stdout().flush();
                last_height = height;
                last_time = Instant::now();
            }
        },
    );

    assert_eq!(pool.panic_count(), 0);

    pool.join();
}
