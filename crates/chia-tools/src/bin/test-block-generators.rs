use clap::Parser;

use chia_bls::PublicKey;
use chia_consensus::gen::conditions::{EmptyVisitor, NewCoin, Spend, SpendBundleConditions};
use chia_consensus::gen::flags::{ALLOW_BACKREFS, MEMPOOL_MODE};
use chia_consensus::gen::run_block_generator::{run_block_generator, run_block_generator2};
use chia_tools::iterate_tx_blocks;
use clvmr::allocator::NodePtr;
use clvmr::serde::{node_from_bytes, node_to_bytes_backrefs};
use clvmr::Allocator;
use std::collections::HashSet;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    #[arg(short, long)]
    file: String,

    /// The number of paralell thread to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// Run all block generators in mempool mode
    #[arg(long, default_value_t = false)]
    mempool: bool,

    /// re-serialize each generator using backrefs and ensure it still produces
    /// the same output
    #[arg(long, default_value_t = false)]
    test_backrefs: bool,

    /// Compare the output from the default ROM running in consensus mode.
    #[arg(short, long, default_value_t = false)]
    validate: bool,

    /// stop running block generators when reaching this height
    #[arg(short, long)]
    max_height: Option<u32>,

    /// start running block generators at this height
    #[arg(long, default_value_t = 0)]
    start_height: u32,

    /// when enabled, run the rust port of the ROM generator
    #[arg(long, default_value_t = false)]
    rust_generator: bool,
}

fn compare_new_coin(a: &Allocator, lhs: &NewCoin, rhs: &NewCoin) {
    assert_eq!(lhs.puzzle_hash, rhs.puzzle_hash);
    assert_eq!(lhs.amount, rhs.amount);
    assert_eq!(a.atom(lhs.hint), a.atom(rhs.hint));
}

fn compare_new_coins(a: &Allocator, lhs: &HashSet<NewCoin>, rhs: &HashSet<NewCoin>) {
    assert_eq!(lhs.len(), rhs.len());

    for c in lhs {
        compare_new_coin(a, c, rhs.get(c).unwrap());
    }
}

fn compare_agg_sig(
    a: &Allocator,
    lhs: &Vec<(PublicKey, NodePtr)>,
    rhs: &Vec<(PublicKey, NodePtr)>,
) {
    assert_eq!(lhs.len(), rhs.len());

    for (l, r) in std::iter::zip(lhs, rhs) {
        assert_eq!(l.0, r.0);
        assert_eq!(a.atom(l.1), a.atom(r.1));
    }
}

fn compare_spend(a: &Allocator, lhs: &Spend, rhs: &Spend) {
    assert_eq!(a.atom(lhs.parent_id), a.atom(rhs.parent_id));
    assert_eq!(lhs.coin_amount, rhs.coin_amount);
    assert_eq!(*lhs.coin_id, *rhs.coin_id);
    assert_eq!(lhs.height_relative, rhs.height_relative);
    assert_eq!(lhs.seconds_relative, rhs.seconds_relative);
    assert_eq!(lhs.before_height_relative, rhs.before_height_relative);
    assert_eq!(lhs.before_seconds_relative, rhs.before_seconds_relative);
    assert_eq!(lhs.birth_height, rhs.birth_height);
    assert_eq!(lhs.birth_seconds, rhs.birth_seconds);
    compare_new_coins(a, &lhs.create_coin, &rhs.create_coin);
    compare_agg_sig(a, &lhs.agg_sig_me, &rhs.agg_sig_me);
    compare_agg_sig(a, &lhs.agg_sig_parent, &rhs.agg_sig_parent);
    compare_agg_sig(a, &lhs.agg_sig_puzzle, &rhs.agg_sig_puzzle);
    compare_agg_sig(a, &lhs.agg_sig_amount, &rhs.agg_sig_amount);
    compare_agg_sig(a, &lhs.agg_sig_puzzle_amount, &rhs.agg_sig_puzzle_amount);
    compare_agg_sig(a, &lhs.agg_sig_parent_amount, &rhs.agg_sig_parent_amount);
    compare_agg_sig(a, &lhs.agg_sig_parent_puzzle, &rhs.agg_sig_parent_puzzle);
    assert_eq!(lhs.flags, rhs.flags);
    assert_eq!(a.atom(lhs.puzzle_hash), a.atom(rhs.puzzle_hash));
}

fn compare_spends(a: &Allocator, lhs: &Vec<Spend>, rhs: &Vec<Spend>) {
    assert_eq!(lhs.len(), rhs.len());

    for (l, r) in std::iter::zip(lhs, rhs) {
        compare_spend(a, l, r);
    }
}

fn compare(a: &Allocator, lhs: &SpendBundleConditions, rhs: &SpendBundleConditions) {
    compare_spends(a, &lhs.spends, &rhs.spends);
    assert_eq!(lhs.reserve_fee, rhs.reserve_fee);
    assert_eq!(lhs.height_absolute, rhs.height_absolute);
    assert_eq!(lhs.seconds_absolute, rhs.seconds_absolute);
    compare_agg_sig(a, &lhs.agg_sig_unsafe, &rhs.agg_sig_unsafe);
    assert_eq!(lhs.before_height_absolute, rhs.before_height_absolute);
    assert_eq!(lhs.before_seconds_absolute, rhs.before_seconds_absolute);
    assert_eq!(lhs.cost, rhs.cost);
    assert_eq!(lhs.removal_amount, rhs.removal_amount);
    assert_eq!(lhs.addition_amount, rhs.addition_amount);
}

fn main() {
    let args = Args::parse();

    if args.validate && !(args.mempool || args.rust_generator || args.test_backrefs) {
        panic!("it doesn't make sense to validate the output against identical runs. Specify --mempool, --rust-generator or --test-backrefs");
    }

    let num_cores = args
        .num_jobs
        .unwrap_or_else(|| available_parallelism().unwrap().into());

    let pool = blocking_threadpool::Builder::new()
        .num_threads(num_cores)
        .queue_len(num_cores + 5)
        .build();

    let flags = if args.mempool { MEMPOOL_MODE } else { 0 }
        | if args.test_backrefs {
            ALLOW_BACKREFS
        } else {
            0
        };

    let block_runner = if args.rust_generator {
        run_block_generator2::<_, EmptyVisitor>
    } else {
        run_block_generator::<_, EmptyVisitor>
    };

    let mut last_height = 0;
    let mut last_time = Instant::now();
    iterate_tx_blocks(
        &args.file,
        args.start_height,
        args.max_height,
        |height, block, block_refs| {
            pool.execute(move || {
                let mut a = Allocator::new_limited(500000000);

                let ti = block.transactions_info.as_ref().expect("transactions_info");
                let prg = block
                    .transactions_generator
                    .as_ref()
                    .expect("transactions_generator");

                let storage: Vec<u8>;
                let generator = if args.test_backrefs {
                    // re-serialize the generator with back-references
                    let gen = node_from_bytes(&mut a, prg.as_ref()).expect("node_from_bytes");
                    storage = node_to_bytes_backrefs(&a, gen).expect("node_to_bytes_backrefs");
                    &storage[..]
                } else {
                    prg.as_ref()
                };

                let mut conditions = block_runner(&mut a, generator, &block_refs, ti.cost, flags)
                    .expect("failed to run block generator");

                if args.rust_generator || args.test_backrefs {
                    assert!(conditions.cost <= ti.cost);
                    assert!(conditions.cost > 0);

                    // in order for the comparison below the hold, we need to
                    // patch up the cost of the rust generator to look like the
                    // baseline
                    conditions.cost = ti.cost;
                } else {
                    assert_eq!(conditions.cost, ti.cost);
                }

                if args.validate {
                    let mut baseline = run_block_generator::<_, EmptyVisitor>(
                        &mut a,
                        prg.as_ref(),
                        &block_refs,
                        ti.cost,
                        0,
                    )
                    .expect("failed to run block generator");
                    assert_eq!(baseline.cost, ti.cost);

                    baseline.spends.sort_by_key(|s| *s.coin_id);
                    conditions.spends.sort_by_key(|s| *s.coin_id);

                    // now ensure the outputs are the same
                    compare(&a, &baseline, &conditions);
                }
            });

            assert_eq!(pool.panic_count(), 0);
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
