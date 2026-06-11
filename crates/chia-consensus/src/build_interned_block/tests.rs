use super::*;
use crate::consensus_constants::TEST_CONSTANTS;
use crate::flags::ConsensusFlags;
use crate::flags::MEMPOOL_MODE;
use crate::owned_conditions::OwnedSpendBundleConditions;
use crate::run_block_generator::run_block_generator2;
use crate::solution_generator::calculate_generator_length;
use crate::spendbundle_conditions::run_spendbundle;
use chia_protocol::{Bytes, Coin};
use chia_traits::Streamable;
use rand::rngs::StdRng;
use rand::{SeedableRng, prelude::SliceRandom};
use std::collections::HashSet;
use std::fs;
use std::time::Instant;

#[test]
fn test_generator_cost_accuracy() {
    // Verify that the upper-bound estimate is always >= the exact cost,
    // and that finalize() returns the correct exact cost.
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    let file = "../../test-bundles/e003f780f1bf036bfa3df7eed6b0e480c2dc3e9d6b1f8c3aeeb542e9da08e8d4.bundle";
    if !std::path::Path::new(file).exists() {
        return;
    }

    let buf = fs::read(file).expect("read bundle file");
    let bundle = SpendBundle::from_bytes(buf.as_slice()).expect("parse SpendBundle");

    let mut a = Allocator::new();
    let conds = run_spendbundle(
        &mut a,
        &bundle,
        11_000_000_000,
        ConsensusFlags::empty(),
        &TEST_CONSTANTS,
    )
    .expect("run_spendbundle")
    .0;

    let cost = conds.cost
        - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
            * TEST_CONSTANTS.cost_per_byte;

    let (added, _) = builder
        .add_spend_bundles([&bundle], cost)
        .expect("add_spend_bundles");
    assert!(added);

    let upper_bound = builder.cost();
    let (_, _, exact_total) = builder.finalize().expect("finalize");

    assert!(
        upper_bound >= exact_total,
        "upper bound {upper_bound} should be >= exact {exact_total}"
    );
}

#[test]
fn test_basic_functionality() {
    // Test basic add and finalize flow
    let builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    assert_eq!(
        builder.cost(),
        WRAPPER_VBYTES * TEST_CONSTANTS.cost_per_byte + 20
    );

    let (generator, sig, cost) = builder.finalize().expect("finalize");

    assert!(!generator.is_empty());
    assert_eq!(sig, Signature::default());
    // Empty builder: block_cost=20 + generator cost of (q . ((nil))) wrapper
    // = 11 vbytes * cost_per_byte + 20
    assert_eq!(cost, 11 * TEST_CONSTANTS.cost_per_byte + 20);
}

fn make_test_coin_spend(parent: [u8; 32], amount: u64) -> chia_protocol::CoinSpend {
    use chia_protocol::{Coin, Program};
    use clvm_utils::tree_hash_from_bytes;

    let puzzle = Program::from(vec![0x01]); // CLVM atom 1
    let solution = Program::from(vec![0x80]); // nil
    let puzzle_hash = tree_hash_from_bytes(puzzle.as_ref())
        .expect("puzzle hash")
        .into();

    chia_protocol::CoinSpend::new(
        Coin::new(parent.into(), puzzle_hash, amount),
        puzzle,
        solution,
    )
}

/// CLVM execution + conditions cost only (excludes generator byte cost).
fn clvm_execution_cost(bundle: &SpendBundle) -> u64 {
    let mut a = Allocator::new();
    let conds = run_spendbundle(
        &mut a,
        bundle,
        11_000_000_000,
        ConsensusFlags::empty(),
        &TEST_CONSTANTS,
    )
    .expect("run_spendbundle")
    .0;
    conds.cost
        - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
            * TEST_CONSTANTS.cost_per_byte
}

/// finalize() must agree with run_block_generator2(..., INTERNED_GENERATOR).
#[test]
fn test_finalize_cost_matches_consensus() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    // Five spends: same puzzle bytes (shared subtree) with different coins.
    let bundles: Vec<SpendBundle> = (0..5)
        .map(|i| {
            SpendBundle::new(
                vec![make_test_coin_spend([i + 1; 32], 1000 + i as u64)],
                Signature::default(),
            )
        })
        .collect();

    for bundle in &bundles {
        let exec_cost = clvm_execution_cost(bundle);
        let (added, _) = builder
            .add_spend_bundles([bundle], exec_cost)
            .expect("add_spend_bundles");
        assert!(added, "bundle should fit");
    }

    let upper_bound = builder.cost();
    let (generator, signature, finalize_cost) = builder.finalize().expect("finalize");

    assert!(
        upper_bound >= finalize_cost,
        "upper bound {upper_bound} must be >= finalize cost {finalize_cost}"
    );

    let (_, conds) = run_block_generator2::<&[u8], _>(
        generator.as_slice(),
        [],
        TEST_CONSTANTS.max_block_cost_clvm,
        MEMPOOL_MODE | ConsensusFlags::INTERNED_GENERATOR,
        &signature,
        None,
        &TEST_CONSTANTS,
    )
    .expect("run_block_generator2");

    assert_eq!(
        conds.cost, finalize_cost,
        "finalize() cost must match consensus INTERNED_GENERATOR path"
    );
}

#[test]
fn test_single_spend_bundle() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    let coin_spend = make_test_coin_spend([1u8; 32], 1000);
    let bundle = SpendBundle::new(vec![coin_spend], Signature::default());

    let (added, result) = builder
        .add_spend_bundles([&bundle], 1000)
        .expect("add_spend_bundles");

    assert!(added, "bundle should be added");
    assert_eq!(result, BuildBlockResult::KeepGoing);

    let (generator, sig, cost) = builder.finalize().expect("finalize");

    assert!(!generator.is_empty(), "generator should not be empty");
    assert_eq!(sig, Signature::default());
    assert!(
        cost > 11 * TEST_CONSTANTS.cost_per_byte + 20,
        "cost should increase from base"
    );
}

#[test]
fn test_cost_accounting() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    let initial_cost = builder.cost();

    let coin_spend1 = make_test_coin_spend([1u8; 32], 1000);
    let bundle1 = SpendBundle::new(vec![coin_spend1], Signature::default());

    let (added, _) = builder
        .add_spend_bundles([&bundle1], 5000)
        .expect("add_spend_bundles");
    assert!(added);

    let cost_after_first = builder.cost();
    assert!(
        cost_after_first > initial_cost,
        "cost should increase after adding bundle"
    );

    let coin_spend2 = make_test_coin_spend([2u8; 32], 2000);
    let bundle2 = SpendBundle::new(vec![coin_spend2], Signature::default());

    let (added, _) = builder
        .add_spend_bundles([&bundle2], 7000)
        .expect("add_spend_bundles");
    assert!(added);

    let cost_after_second = builder.cost();
    assert!(
        cost_after_second > cost_after_first,
        "cost should increase after adding second bundle"
    );
}

#[test]
fn test_block_full_overflow() {
    let small_max = 100_000;
    let mut builder = InternedBlockBuilder::new_with(TEST_CONSTANTS.cost_per_byte, small_max);

    let coin_spend = make_test_coin_spend([1u8; 32], 1000);
    let bundle = SpendBundle::new(vec![coin_spend], Signature::default());

    let (added, result) = builder
        .add_spend_bundles([&bundle], small_max - 5000)
        .expect("add_spend_bundles");

    if added {
        assert_eq!(
            result,
            BuildBlockResult::Done,
            "should signal Done when close to limit"
        );
    } else {
        assert_eq!(
            result,
            BuildBlockResult::Done,
            "should be Done if bundle doesn't fit"
        );
    }
}

#[test]
fn test_num_skipped() {
    let small_max = 50_000;
    let mut builder = InternedBlockBuilder::new_with(TEST_CONSTANTS.cost_per_byte, small_max);

    let mut num_rejected = 0;

    for i in 0..10 {
        let coin_spend = make_test_coin_spend([i; 32], 1000);
        let bundle = SpendBundle::new(vec![coin_spend], Signature::default());

        let (added, result) = builder
            .add_spend_bundles([&bundle], 10_000)
            .expect("add_spend_bundles");

        if !added {
            num_rejected += 1;
        }

        if result == BuildBlockResult::Done {
            break;
        }
    }

    assert!(num_rejected > 0, "some bundles should have been skipped");
    assert!(
        num_rejected <= MAX_SKIPPED_ITEMS as usize + 1,
        "should stop after MAX_SKIPPED_ITEMS"
    );
}

#[test]
fn test_byte_cost_ub_tracking() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    assert_eq!(builder.byte_cost_ub, 0, "byte_cost_ub should start at 0");

    let coin_spend = make_test_coin_spend([1u8; 32], 1000);
    let bundle = SpendBundle::new(vec![coin_spend], Signature::default());

    let (added, _) = builder
        .add_spend_bundles([&bundle], 5000)
        .expect("add_spend_bundles");
    assert!(added);

    assert!(
        builder.byte_cost_ub > 0,
        "byte_cost_ub should increase after adding bundle"
    );

    let initial_byte_cost = builder.byte_cost_ub;

    let coin_spend2 = make_test_coin_spend([2u8; 32], 2000);
    let bundle2 = SpendBundle::new(vec![coin_spend2], Signature::default());

    let (added, _) = builder
        .add_spend_bundles([&bundle2], 7000)
        .expect("add_spend_bundles");
    assert!(added);

    assert!(
        builder.byte_cost_ub > initial_byte_cost,
        "byte_cost_ub should increase with each bundle"
    );

    let upper_bound = builder.cost();
    let (_, _, exact_cost) = builder.finalize().expect("finalize");

    assert!(
        upper_bound >= exact_cost,
        "upper bound ({upper_bound}) should be >= exact cost ({exact_cost})"
    );
}

#[ignore = "expensive test, only run in release mode (--include-ignored)"]
#[test]
fn test_build_interned_block() {
    let mut all_bundles = vec![];
    println!("loading spend bundles from disk");
    let mut seen_spends = HashSet::new();
    for entry in fs::read_dir("../../test-bundles").expect("listing test-bundles directory") {
        let file = entry.expect("list dir").path();
        if file.extension().map(|s| s.to_str()) != Some(Some("bundle")) {
            continue;
        }
        // only use 32 byte hex encoded filenames
        if file.file_stem().map(std::ffi::OsStr::len) != Some(64_usize) {
            continue;
        }
        let buf = fs::read(file.clone()).expect("read bundle file");
        let bundle = SpendBundle::from_bytes(buf.as_slice()).expect("parsing SpendBundle");

        let mut a = Allocator::new();
        let conds = run_spendbundle(
            &mut a,
            &bundle,
            11_000_000_000,
            ConsensusFlags::empty(),
            &TEST_CONSTANTS,
        )
        .expect("run_spendbundle")
        .0;

        if conds
            .spends
            .iter()
            .any(|s| seen_spends.contains(&*s.coin_id))
        {
            panic!(
                "conflict in {}",
                file.file_name().unwrap().to_str().unwrap()
            );
        }
        if conds.spends.iter().any(|s| {
            s.create_coin.iter().any(|c| {
                seen_spends.contains(&Coin::new(*s.coin_id, c.puzzle_hash, c.amount).coin_id())
            })
        }) {
            panic!(
                "conflict in {}",
                file.file_name().unwrap().to_str().unwrap()
            );
        }
        for spend in &conds.spends {
            seen_spends.insert(*spend.coin_id);
            for coin in &spend.create_coin {
                seen_spends
                    .insert(Coin::new(*spend.coin_id, coin.puzzle_hash, coin.amount).coin_id());
            }
        }

        let cost = conds.cost
            - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
                * TEST_CONSTANTS.cost_per_byte;

        let mut conds = OwnedSpendBundleConditions::from(&a, conds);
        for s in &mut conds.spends {
            s.flags = 0;
            s.fingerprint = Bytes::default();
            s.create_coin.sort();
        }
        all_bundles.push(Box::new((bundle, cost, conds)));
    }
    all_bundles.sort_by_key(|x| x.1);
    println!("loaded {} spend bundles", all_bundles.len());

    let num_cores: usize = std::thread::available_parallelism().unwrap().into();
    let pool = blocking_threadpool::Builder::new()
        .num_threads(num_cores)
        .queue_len(num_cores + 1)
        .build();

    for seed in 0..30 {
        let mut bundles = all_bundles.clone();
        let mut rng = StdRng::seed_from_u64(seed);
        pool.execute(move || {
            bundles.shuffle(&mut rng);

            let start = Instant::now();
            let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);
            let mut skipped = 0;
            let mut num_tx = 0;
            let mut max_call_time = 0.0f32;
            let mut spends = vec![];
            for entry in &bundles {
                let (bundle, cost, conds) = entry.as_ref();
                let start_call = Instant::now();
                let (added, result) = builder
                    .add_spend_bundles([bundle], *cost)
                    .expect("add_spend_bundle");

                max_call_time = f32::max(max_call_time, start_call.elapsed().as_secs_f32());
                if added {
                    num_tx += 1;
                    spends.extend(conds.spends.iter());
                } else {
                    skipped += 1;
                }
                if result == BuildBlockResult::Done {
                    break;
                }
            }
            let upper_bound_cost = builder.cost();
            let (generator, signature, cost) = builder.finalize().expect("finalize()");

            assert!(
                upper_bound_cost >= cost,
                "upper bound {upper_bound_cost} must be >= exact cost {cost}"
            );

            println!(
                "idx: {seed:3} built block in {:>5.2} seconds, cost: {cost:11} skipped: {skipped:2} longest-call: {max_call_time:>5.2}s TX: {num_tx}",
                start.elapsed().as_secs_f32()
            );

            let (a, conds) = run_block_generator2::<&[u8], _>(
                generator.as_slice(),
                [],
                TEST_CONSTANTS.max_block_cost_clvm,
                MEMPOOL_MODE | ConsensusFlags::INTERNED_GENERATOR,
                &signature,
                None,
                &TEST_CONSTANTS,
            )
            .expect("run_block_generator2");
            assert_eq!(conds.cost, cost);
            let mut conds = OwnedSpendBundleConditions::from(&a, conds);

            assert_eq!(conds.spends.len(), spends.len());
            conds.spends.sort_by_key(|s| s.coin_id);
            spends.sort_by_key(|s| s.coin_id);
            for (mut generator, tx) in conds.spends.into_iter().zip(spends) {
                generator.create_coin.sort();
                generator.flags = 0;
                generator.fingerprint = Bytes::default();
                assert_eq!(&generator, tx);
            }
        });
        assert_eq!(pool.panic_count(), 0);
    }
    pool.join();
    assert_eq!(pool.panic_count(), 0);
}
