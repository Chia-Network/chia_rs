// This is a throw-away tool to scan a chia blockchain database for spends that
// look like they might support "fast-forward" (or rebasing on top of a new
// coin). It was used to pull out real-life examples of spends that would
// satisfy the requirements on the fast-foward feature.

use chia_puzzles::SINGLETON_TOP_LAYER_V1_1_HASH;
use clap::Parser;

use chia_tools::{iterate_blocks, visit_spends};
use chia_traits::streamable::Streamable;

use chia_bls::G2Element;
use chia_protocol::{Bytes32, Coin, CoinSpend, Program, SpendBundle};
use clvm_traits::FromClvm;
use clvm_utils::{CurriedProgram, tree_hash};
use clvmr::Allocator;
use clvmr::allocator::NodePtr;
use core::sync::atomic::Ordering;
use std::collections::HashSet;
use std::fs::{File, write};
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

/// Analyze the spends in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Path to blockchain database file to analyze
    file: String,

    /// The number of threads to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// generate spend-bundles
    #[arg(long, default_value_t = false)]
    spend_bundles: bool,

    /// generate corpus for run_block_generator() and additions_and_removals()
    #[arg(long, default_value_t = false)]
    block_generators: bool,

    /// generate corpus for solution-generator
    #[arg(long, default_value_t = false)]
    coin_spends: bool,

    /// generate corpus for run-puzzle
    #[arg(long, default_value_t = false)]
    puzzles: bool,

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

    let seen_puzzles = Arc::new(Mutex::new(HashSet::<Bytes32>::new()));
    let seen_singletons = Arc::new(Mutex::new(HashSet::<Bytes32>::new()));

    let mut last_height = 0;
    let mut last_time = Instant::now();
    let corpus_counter = Arc::new(AtomicUsize::new(0));
    iterate_blocks(
        &args.file,
        args.start_height,
        args.max_height,
        |height, block, block_refs| {
            if block.transactions_generator.is_none() {
                return;
            }
            // this is called for each transaction block
            let max_cost = block.transactions_info.unwrap().cost;
            let prg = block.transactions_generator.unwrap();

            let seen_puzzles = seen_puzzles.clone();
            let seen_singletons = seen_singletons.clone();
            let cnt = corpus_counter.clone();
            pool.execute(move || {
                let mut a = Allocator::new_limited(500_000_000);

                let generator = prg.as_ref();

                if args.block_generators {
                    let directory = "../chia-protocol/fuzz/corpus/run-generator";
                    let _ = std::fs::create_dir_all(directory);
                    write(format!("{directory}/{height}.bundle"), generator).expect("write");

                    let directory = "../chia-protocol/fuzz/corpus/additions-and-removals";
                    let _ = std::fs::create_dir_all(directory);
                    write(format!("{directory}/{height}.bundle"), generator).expect("write");
                    cnt.fetch_add(1, Ordering::Relaxed);
                }
                let mut bundle = SpendBundle::new(vec![], G2Element::default());

                if args.puzzles || args.spend_bundles || args.coin_spends {
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

                            let seen_puzzle = seen_puzzles.lock().unwrap().insert(mod_hash);
                            let run_puzzle = args.puzzles && seen_puzzle;
                            let fast_forward = mod_hash == SINGLETON_TOP_LAYER_V1_1_HASH.into()
                                && seen_singletons.lock().unwrap().insert(puzzle_hash);

                            if !run_puzzle
                                && !fast_forward
                                && !args.spend_bundles
                                && !args.coin_spends
                            {
                                return;
                            }
                            let puzzle_reveal =
                                Program::from_clvm(a, puzzle).expect("puzzle reveal");
                            let solution = Program::from_clvm(a, solution).expect("solution");
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

                            if (args.spend_bundles || args.coin_spends) && !seen_puzzle {
                                bundle.coin_spends.push(spend.clone());
                            }

                            if !run_puzzle && !fast_forward {
                                return;
                            }
                            let bytes = spend.to_bytes().expect("stream CoinSpend");
                            if run_puzzle {
                                let directory = "../chia-consensus/fuzz/corpus/run-puzzle";
                                let _ = std::fs::create_dir_all(directory);
                                write(format!("{directory}/{mod_hash}.spend"), &bytes)
                                    .expect("write");
                                cnt.fetch_add(1, Ordering::Relaxed);
                            }

                            if fast_forward {
                                let directory = "../chia-consensus/fuzz/corpus/fast-forward";
                                let _ = std::fs::create_dir_all(directory);
                                write(format!("{directory}/{puzzle_hash}.spend"), &bytes)
                                    .expect("write");
                                cnt.fetch_add(1, Ordering::Relaxed);
                            }
                        },
                    )
                    .expect("failed to run block generator");
                }

                if args.spend_bundles && !bundle.coin_spends.is_empty() {
                    let directory = "../chia-protocol/fuzz/corpus/spend-bundle";
                    let _ = std::fs::create_dir_all(directory);
                    let bytes = bundle.to_bytes().expect("to_bytes");
                    write(format!("{directory}/{height}.bundle"), bytes).expect("write");
                    cnt.fetch_add(1, Ordering::Relaxed);
                }

                if args.coin_spends && !bundle.coin_spends.is_empty() {
                    let directory = "../chia-consensus/fuzz/corpus/solution-generator";
                    let _ = std::fs::create_dir_all(directory);
                    let mut f =
                        File::create(format!("{directory}/{height}.spends")).expect("open file");
                    for cs in &bundle.coin_spends {
                        f.write_all(&cs.to_bytes().expect("CoinSpend serialize"))
                            .expect("file write");
                    }
                    cnt.fetch_add(1, Ordering::Relaxed);
                }
            });
            if last_time.elapsed() > Duration::new(4, 0) {
                let rate = f64::from(height - last_height) / last_time.elapsed().as_secs_f64();
                print!(
                    "\rheight: {height} ({rate:0.1} blocks/s) corpus: {}    ",
                    corpus_counter.load(Ordering::Relaxed)
                );
                let _ = std::io::stdout().flush();
                last_height = height;
                last_time = Instant::now();
            }
        },
    );
    print!(
        "\nwrote {} examples to the fuzzing corpus",
        corpus_counter.load(Ordering::Relaxed)
    );

    assert_eq!(pool.panic_count(), 0);

    pool.join();
}
