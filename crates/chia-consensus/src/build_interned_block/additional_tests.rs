use super::*;
use crate::consensus_constants::TEST_CONSTANTS;
use crate::flags::ConsensusFlags;
use crate::flags::MEMPOOL_MODE;
use crate::run_block_generator::run_block_generator2;
use crate::solution_generator::{calculate_generator_length, solution_generator_backrefs};
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
    let bundles = serde_2026_test_bundles();

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

/// Deterministic set of bundles with shared puzzle bytes, used by the
/// serde_2026 emission tests below.
fn serde_2026_test_bundles() -> Vec<SpendBundle> {
    (0..5)
        .map(|i| {
            SpendBundle::new(
                vec![make_test_coin_spend([i + 1; 32], 1000 + i as u64)],
                Signature::default(),
            )
        })
        .collect()
}

fn build_block(bundles: &[SpendBundle]) -> (Vec<u8>, Signature, u64) {
    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);
    for bundle in bundles {
        let exec_cost = clvm_execution_cost(bundle);
        let (added, _) = builder
            .add_spend_bundles([bundle], exec_cost)
            .expect("add_spend_bundles");
        assert!(added, "bundle should fit");
    }
    builder.finalize().expect("finalize")
}

/// Independently constructs the classic-serialization reference generator
/// for the same spend list the builder would build, without going through
/// `InternedBlockBuilder` (which only ever emits serde_2026). Used to compare
/// serde_2026 output against a classic-format generator for the same
/// bundles, run under classic (pre-HF2) consensus rules. Delegates the
/// actual encoding to `solution_generator_backrefs`, the same-crate function
/// the mempool uses for this — the tuple conversion and signature
/// aggregation here are the only test-specific bits.
fn build_classic_reference(bundles: &[SpendBundle]) -> (Vec<u8>, Signature) {
    let mut signature = Signature::default();
    let mut spends = Vec::new();
    for bundle in bundles {
        for spend in &bundle.coin_spends {
            spends.push((
                spend.coin,
                spend.puzzle_reveal.as_ref(),
                spend.solution.as_ref(),
            ));
        }
        signature.aggregate(&bundle.aggregated_signature);
    }
    let generator = solution_generator_backrefs(spends).expect("solution_generator_backrefs");
    (generator, signature)
}

fn normalized_spends(
    generator: &[u8],
    signature: &Signature,
    flags: ConsensusFlags,
) -> (Vec<crate::owned_conditions::OwnedSpendConditions>, u64) {
    let (a, conds) = run_block_generator2::<&[u8], _>(
        generator,
        [],
        TEST_CONSTANTS.max_block_cost_clvm,
        MEMPOOL_MODE | flags,
        signature,
        None,
        &TEST_CONSTANTS,
    )
    .expect("run_block_generator2");
    let cost = conds.cost;
    let mut conds = crate::owned_conditions::OwnedSpendBundleConditions::from(&a, conds);
    conds.spends.sort_by_key(|s| s.coin_id);
    for s in &mut conds.spends {
        s.create_coin.sort();
        s.flags = 0;
        s.fingerprint = chia_protocol::Bytes::default();
    }
    (conds.spends, cost)
}

/// The builder's serde_2026 output round-trips through the
/// INTERNED_GENERATOR consensus path and yields the same spends/conditions
/// as an independently-built classic generator for the same bundles, run
/// under classic rules.
#[test]
fn test_serde_2026_round_trip() {
    use clvmr::serde::SERDE_2026_MAGIC_PREFIX;

    let bundles = serde_2026_test_bundles();

    let (generator_2026, sig_2026, cost_2026) = build_block(&bundles);
    assert!(
        generator_2026.starts_with(&SERDE_2026_MAGIC_PREFIX),
        "builder output must always carry the serde_2026 magic prefix"
    );

    let (generator_classic, sig_classic) = build_classic_reference(&bundles);
    assert!(!generator_classic.starts_with(&SERDE_2026_MAGIC_PREFIX));

    let (spends_2026, run_cost_2026) = normalized_spends(
        &generator_2026,
        &sig_2026,
        ConsensusFlags::INTERNED_GENERATOR,
    );
    assert_eq!(
        run_cost_2026, cost_2026,
        "finalize() cost must match the INTERNED_GENERATOR consensus path"
    );

    // classic reference generator, run under classic (pre-HF2) rules
    let (spends_classic, _) =
        normalized_spends(&generator_classic, &sig_classic, ConsensusFlags::empty());

    assert_eq!(spends_2026, spends_classic);
}

/// tree_hash_2026 semantics: hashing the serde_2026 generator agrees with the
/// tree hash of the classic serialization of the same tree.
#[test]
fn test_serde_2026_tree_hash_2026_agrees() {
    use crate::serde_2026::node_from_bytes_2026_trusted;
    use clvm_utils::{tree_hash, tree_hash_from_bytes};

    let bundles = serde_2026_test_bundles();
    let (generator_2026, _, _) = build_block(&bundles);
    let (generator_classic, _) = build_classic_reference(&bundles);

    // same parse as the wheel's tree_hash_2026()
    let mut a = Allocator::new();
    let node = node_from_bytes_2026_trusted(&mut a, &generator_2026)
        .expect("node_from_bytes_2026_trusted");
    let hash_2026 = tree_hash(&a, node);

    let hash_classic = tree_hash_from_bytes(&generator_classic).expect("tree_hash_from_bytes");
    assert_eq!(hash_2026, hash_classic);
}
