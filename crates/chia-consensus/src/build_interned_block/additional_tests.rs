use super::*;
use crate::consensus_constants::TEST_CONSTANTS;
use crate::flags::ConsensusFlags;
use crate::flags::MEMPOOL_MODE;
use crate::run_block_generator::run_block_generator2;
use crate::solution_generator::calculate_generator_length;
use crate::spendbundle_conditions::run_spendbundle;
use chia_traits::Streamable;
use std::fs;
use std::path::Path;

/// For a single spend bundle: upper bound >= finalize cost, and finalize cost matches
/// `run_block_generator2(..., INTERNED_GENERATOR)` (block header cost).
fn assert_generator_cost_accuracy(bundle: &SpendBundle) {
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

    let cost = conds.cost
        - (calculate_generator_length(&bundle.coin_spends) as u64 - 2)
            * TEST_CONSTANTS.cost_per_byte;

    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);
    let (added, _) = builder
        .add_spend_bundles([bundle], cost)
        .expect("add_spend_bundles");
    assert!(added, "bundle should fit in block");

    let upper_bound = builder.cost();
    let (generator, signature, exact_total) = builder.finalize().expect("finalize");

    assert!(
        upper_bound >= exact_total,
        "upper bound {upper_bound} should be >= exact {exact_total}"
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
        conds.cost, exact_total,
        "finalize() cost must match consensus INTERNED_GENERATOR path"
    );
}

#[test]
fn test_generator_cost_accuracy() {
    // Same hex-named fixtures as test_build_block (90 bundles). Each is checked as a
    // single-bundle block so aggregate signatures validate under run_block_generator2.
    let dir = Path::new("../../test-bundles");
    let mut count = 0;
    for entry in fs::read_dir(dir).expect("listing test-bundles directory") {
        let file = entry.expect("list dir").path();
        if file.extension().and_then(|s| s.to_str()) != Some("bundle") {
            continue;
        }
        // only use 32 byte hex encoded filenames
        if file.file_stem().map(std::ffi::OsStr::len) != Some(64_usize) {
            continue;
        }

        let buf = fs::read(&file).expect("read bundle file");
        let bundle = SpendBundle::from_bytes(buf.as_slice()).expect("parse SpendBundle");
        assert_generator_cost_accuracy(&bundle);
        count += 1;
    }
    assert_eq!(count, 90, "expected 90 hex-named test bundles");
}

#[test]
fn test_basic_functionality() {
    // Test basic add and finalize flow
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

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
    assert!(result == BuildBlockResult::KeepGoing);

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

    assert!(!added, "bundle should not fit when block is near full");
    assert!(result == BuildBlockResult::Done);
}

#[test]
fn test_num_skipped() {
    let cost_per_byte = TEST_CONSTANTS.cost_per_byte;
    // Room for MIN_COST_THRESHOLD, but individual bundles can still be rejected.
    let max = MIN_COST_THRESHOLD + WRAPPER_VBYTES * cost_per_byte + 20 + 1_000_000;

    let mut builder = InternedBlockBuilder::new_with(cost_per_byte, max);

    let coin_spend = make_test_coin_spend([1u8; 32], 1000);
    let bundle = SpendBundle::new(vec![coin_spend], Signature::default());

    // Declared CLVM cost alone exceeds max (rejected before parsing spends).
    let declared_cost = max - WRAPPER_VBYTES * cost_per_byte - 20 + 1;

    for _ in 0..MAX_SKIPPED_ITEMS {
        let (added, result) = builder
            .add_spend_bundles([&bundle], declared_cost)
            .expect("add_spend_bundles");
        assert!(!added);
        assert!(result == BuildBlockResult::KeepGoing);
    }

    let (added, result) = builder
        .add_spend_bundles([&bundle], declared_cost)
        .expect("add_spend_bundles");
    assert!(!added);
    assert!(result == BuildBlockResult::Done);
}

/// finalize() must reset the builder to a fresh state on success: a second
/// finalize() call (with no intervening add_spend_bundles()) must produce
/// the same empty-spend-list generator, cost, and signature as a brand new
/// builder — not a generator that still contains the first call's spends
/// under a signature that no longer covers them.
#[test]
fn test_finalize_resets_builder() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    let coin_spend = make_test_coin_spend([1u8; 32], 1000);
    let bundle = SpendBundle::new(vec![coin_spend], Signature::default());
    let (added, _) = builder
        .add_spend_bundles([&bundle], 5000)
        .expect("add_spend_bundles");
    assert!(added);

    let (first_generator, _, first_cost) = builder.finalize().expect("finalize");
    assert!(
        first_cost > 11 * TEST_CONSTANTS.cost_per_byte + 20,
        "first finalize should reflect the added spend"
    );

    // The builder was reset by the first finalize(), so this second call
    // (with nothing added in between) must behave like a fresh builder.
    let (second_generator, second_sig, second_cost) = builder.finalize().expect("finalize");
    assert_eq!(second_sig, Signature::default());
    assert_eq!(
        second_cost,
        11 * TEST_CONSTANTS.cost_per_byte + 20,
        "second finalize should be the empty-spend-list cost"
    );
    assert_ne!(
        first_generator, second_generator,
        "second generator must not still contain the first call's spend"
    );

    let mut fresh = InternedBlockBuilder::new(&TEST_CONSTANTS);
    let (fresh_generator, fresh_sig, fresh_cost) = fresh.finalize().expect("finalize");
    assert_eq!(second_generator, fresh_generator);
    assert_eq!(second_sig, fresh_sig);
    assert_eq!(second_cost, fresh_cost);
}

/// add_spend_bundles() after finalize() must start from a clean slate: the
/// resulting generator/signature/cost must be identical to a brand new
/// builder that only ever saw the second bundle.
#[test]
fn test_add_after_finalize_starts_clean() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    let coin_spend1 = make_test_coin_spend([1u8; 32], 1000);
    let bundle1 = SpendBundle::new(vec![coin_spend1], Signature::default());
    let (added, _) = builder
        .add_spend_bundles([&bundle1], 5000)
        .expect("add_spend_bundles");
    assert!(added);
    builder.finalize().expect("finalize");

    let coin_spend2 = make_test_coin_spend([2u8; 32], 2000);
    let bundle2 = SpendBundle::new(vec![coin_spend2], Signature::default());
    let (added, _) = builder
        .add_spend_bundles([&bundle2], 7000)
        .expect("add_spend_bundles");
    assert!(added);
    let (generator, sig, cost) = builder.finalize().expect("finalize");

    let mut fresh = InternedBlockBuilder::new(&TEST_CONSTANTS);
    let (added, _) = fresh
        .add_spend_bundles([&bundle2], 7000)
        .expect("add_spend_bundles");
    assert!(added);
    let (fresh_generator, fresh_sig, fresh_cost) = fresh.finalize().expect("finalize");

    assert_eq!(
        generator, fresh_generator,
        "generator should not carry over bundle1"
    );
    assert_eq!(sig, fresh_sig);
    assert_eq!(cost, fresh_cost);
}

#[test]
fn test_byte_cost_tracking() {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);

    assert_eq!(builder.byte_cost, 0, "byte_cost should start at 0");

    let coin_spend = make_test_coin_spend([1u8; 32], 1000);
    let bundle = SpendBundle::new(vec![coin_spend], Signature::default());

    let (added, _) = builder
        .add_spend_bundles([&bundle], 5000)
        .expect("add_spend_bundles");
    assert!(added);

    assert!(
        builder.byte_cost > 0,
        "byte_cost should increase after adding bundle"
    );

    let initial_byte_cost = builder.byte_cost;

    let coin_spend2 = make_test_coin_spend([2u8; 32], 2000);
    let bundle2 = SpendBundle::new(vec![coin_spend2], Signature::default());

    let (added, _) = builder
        .add_spend_bundles([&bundle2], 7000)
        .expect("add_spend_bundles");
    assert!(added);

    assert!(
        builder.byte_cost > initial_byte_cost,
        "byte_cost should increase with each bundle"
    );

    let upper_bound = builder.cost();
    let (_, _, exact_cost) = builder.finalize().expect("finalize");

    assert!(
        upper_bound >= exact_cost,
        "upper bound ({upper_bound}) should be >= exact cost ({exact_cost})"
    );
}
