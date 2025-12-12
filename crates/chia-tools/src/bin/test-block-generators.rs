use clap::Parser;

use chia_bls::PublicKey;
use chia_consensus::conditions::{NewCoin, SpendBundleConditions, SpendConditions};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::{DONT_VALIDATE_SIGNATURE, MEMPOOL_MODE};
use chia_consensus::run_block_generator::{
    get_coinspends_for_trusted_block, run_block_generator, run_block_generator2,
};
use chia_protocol::Program;
use chia_tools::iterate_blocks;
use clvmr::Allocator;
use clvmr::allocator::NodePtr;
use clvmr::serde::Serializer;
use clvmr::serde::{is_canonical_serialization, node_from_bytes_backrefs};
use std::collections::HashSet;
use std::io::Write;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Path to blockchain database file to analyze
    file: String,

    /// The number of paralell thread to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// Run all block generators in mempool mode
    #[arg(long, default_value_t = false)]
    mempool: bool,

    /// Don't validate block signatures (saves time)
    #[arg(long, default_value_t = false)]
    skip_signature_validation: bool,

    /// recompress generators with Serializer class and report any discrepancy.
    /// This cannot be combined with --original-generator
    #[arg(long, default_value_t = false)]
    test_serializer: bool,

    /// check all block generators to see if any is using overlong encoding
    #[arg(long, default_value_t = false)]
    test_canonical_encoding: bool,

    /// Compare the output from the default ROM running in consensus mode
    /// against the hard-fork rules for executing block generators. After the
    /// hard fork, the CLVM ROM implementation is no longer expected to work, so
    /// this option also implies max-height=hard-fork-height.
    #[arg(short, long, default_value_t = false)]
    original_generator: bool,

    /// The hard fork block height. Defaults to mainnet (5,496,000). For
    /// testnet11, set to 0.
    #[arg(long, default_value_t = 5_496_000)]
    hard_fork_height: u32,

    /// stop running block generators when reaching this height
    #[arg(short, long)]
    max_height: Option<u32>,

    /// start running block generators at this height
    #[arg(long, default_value_t = 0)]
    start_height: u32,
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

fn compare_spend(a: &Allocator, lhs: &SpendConditions, rhs: &SpendConditions) {
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

fn compare_spends(a: &Allocator, lhs: &Vec<SpendConditions>, rhs: &Vec<SpendConditions>) {
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

    // TODO: Use the real consants here
    let constants = &TEST_CONSTANTS;

    let num_cores = args
        .num_jobs
        .unwrap_or_else(|| available_parallelism().unwrap().into());

    let pool = blocking_threadpool::Builder::new()
        .num_threads(num_cores)
        .queue_len(num_cores + 5)
        .build();

    let flags = if args.mempool { MEMPOOL_MODE } else { 0 };

    // Blocks created after the hard fork are not expected to work with the
    // original generator ROM. The cost will exceed the block cost. So when
    // validating blocks using the old generator, we have to stop at the hard
    // fork.
    let max_height = if args.original_generator {
        Some(args.hard_fork_height)
    } else {
        args.max_height
    };

    if let Some(h) = max_height {
        if args.start_height >= h {
            println!(
                "start height ({}) is greater than max height {h})",
                args.start_height
            );
            return;
        }
    }

    let mut last_height = args.start_height;
    let mut last_time = Instant::now();
    println!("opening blockchain database file: {}", args.file);
    iterate_blocks(
        &args.file,
        args.start_height,
        max_height,
        |height, block, block_refs| {
            if block.transactions_generator.is_none() {
                return;
            }
            pool.execute(move || {
                let mut a = Allocator::new_limited(500_000_000);

                let ti = block.transactions_info.as_ref().expect("transactions_info");
                let generator = block
                    .transactions_generator
                    .as_ref()
                    .expect("transactions_generator");

                if args.test_canonical_encoding {
                    if !is_canonical_serialization(generator) {
                        println!("generator at height {height} uses overlong CLVM encoding");
                    }
                    return;
                }

                // after the hard fork, we run blocks without paying for the
                // CLVM generator ROM
                let block_runner = if args.original_generator || height >= args.hard_fork_height {
                    run_block_generator2
                } else {
                    run_block_generator
                };
                let flags = flags
                    | if args.skip_signature_validation {
                        DONT_VALIDATE_SIGNATURE
                    } else {
                        0
                    };
                let mut conditions = block_runner(
                    &mut a,
                    generator,
                    &block_refs,
                    ti.cost,
                    flags,
                    &ti.aggregated_signature,
                    None,
                    constants,
                )
                .expect("failed to run block generator");

                if args.test_serializer {
                    let new_gen = {
                        // this is a temporary (local) allocator, just for the
                        // purposes of re-serializing the block
                        let mut a = Allocator::new();
                        let generator_ptr = node_from_bytes_backrefs(&mut a, generator)
                            .expect("deserialize generator");
                        let mut ser = Serializer::new(None);
                        let (done, _) = ser.add(&a, generator_ptr).expect("serialize");
                        assert!(done);
                        let new_gen = ser.into_inner();
                        if new_gen.len() > generator.len() + 4 {
                            println!(
                                "height: {height} orig: {} new: {} ratio: {:0.6} diff: {}",
                                generator.len(),
                                new_gen.len(),
                                new_gen.len() as f64 / generator.len() as f64,
                                new_gen.len() as i64 - generator.len() as i64
                            );
                        }
                        new_gen
                    };

                    let cost_offset = (new_gen.len() as i64 - generator.len() as i64)
                        * constants.cost_per_byte as i64;
                    let new_cost = (ti.cost as i64 + cost_offset) as u64;
                    // since we just compressed the block, we have to run it
                    // with the new run_block_generator
                    let prog = &Program::new(new_gen.into());
                    let mut recompressed_conditions = run_block_generator2(
                        &mut a,
                        prog,
                        &block_refs,
                        new_cost,
                        flags,
                        &ti.aggregated_signature,
                        None,
                        constants,
                    )
                    .expect("failed to run block generator");

                    recompressed_conditions.spends.sort_by_key(|s| *s.coin_id);
                    conditions.spends.sort_by_key(|s| *s.coin_id);

                    // in order for the comparison below the hold, we need to
                    // patch up the cost of the rust generator to look like the
                    // baseline
                    recompressed_conditions.cost = ti.cost;

                    // now ensure the outputs are the same
                    compare(&a, &recompressed_conditions, &conditions);

                    // now lets check get_coinspends_for_trusted_block
                    let vec_of_slices: Vec<&[u8]> =
                        block_refs.iter().map(std::vec::Vec::as_slice).collect();

                    let coinspends =
                        get_coinspends_for_trusted_block(constants, prog, &vec_of_slices, flags)
                            .expect("get_coinspends");
                    for (i, spend) in recompressed_conditions.spends.into_iter().enumerate() {
                        let parent_id = a.atom(spend.parent_id);
                        assert_eq!(
                            parent_id.as_ref(),
                            coinspends[i].coin.parent_coin_info.as_slice()
                        );
                        let puzhash = a.atom(spend.puzzle_hash);
                        assert_eq!(puzhash.as_ref(), coinspends[i].coin.puzzle_hash.as_slice());
                        assert_eq!(spend.coin_amount, coinspends[i].coin.amount);
                    }
                    return;
                }

                if args.original_generator && height < args.hard_fork_height {
                    // when running pre-hardfork blocks with the post-hard fork
                    // generator, we get a lower cost than what's recorded in
                    // the block. Because the new generator is cheaper.
                    assert!(conditions.cost <= ti.cost);
                    assert!(conditions.cost > 0);

                    // in order for the comparison below the hold, we need to
                    // patch up the cost of the rust generator to look like the
                    // baseline
                    conditions.cost = ti.cost;
                } else {
                    assert_eq!(conditions.cost, ti.cost);
                }

                if args.original_generator {
                    let mut baseline = run_block_generator(
                        &mut a,
                        generator.as_ref(),
                        &block_refs,
                        ti.cost,
                        flags,
                        &ti.aggregated_signature,
                        None,
                        constants,
                    )
                    .expect("run_block_generator()");
                    assert_eq!(baseline.cost, ti.cost);

                    baseline.spends.sort_by_key(|s| *s.coin_id);
                    conditions.spends.sort_by_key(|s| *s.coin_id);

                    // now ensure the outputs are the same
                    compare(&a, &baseline, &conditions);
                }
            });

            assert_eq!(pool.panic_count(), 0);
            if last_time.elapsed() > Duration::new(2, 0) {
                let rate = f64::from(height - last_height) / last_time.elapsed().as_secs_f64();
                print!("\rheight: {height} ({rate:0.1} blocks/s)   ");
                let _ = std::io::stdout().flush();
                last_height = height;
                last_time = Instant::now();
            }
        },
    );

    pool.join();
    assert_eq!(pool.panic_count(), 0);

    println!("ALL DONE, success!");
}
