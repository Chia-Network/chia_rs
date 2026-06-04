#![no_main]
use chia_consensus::conditions::{
    ConditionsCache, EmptyVisitor, ParseState, SpendBundleConditions, process_single_spend,
    validate_conditions,
};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::owned_conditions::OwnedSpendBundleConditions;
use chia_consensus::validation_error::ErrorCode;
use clvm_fuzzing::ArbitraryClvmTree;
use libfuzzer_sys::fuzz_target;

const NUM_SPENDS: usize = 4;

fuzz_target!(|args: (ArbitraryClvmTree, [[u8; 32]; NUM_SPENDS])| {
    let (conds, parent_ids) = args;
    let mut a = conds.allocator;

    let puzzle_hash_bytes = [0xab_u8; 32];
    let puzzle_hash = a.new_atom(&puzzle_hash_bytes).expect("new_atom");
    let amount = a.new_number(1u64.into()).expect("new_atom");

    let parent_nodes: Vec<_> = parent_ids
        .iter()
        .map(|id| a.new_atom(id).expect("new_atom"))
        .collect();

    // Run A: without cache
    let mut ret_a = SpendBundleConditions::default();
    let mut state_a = ParseState::default();
    let mut errors_a: Vec<Option<ErrorCode>> = Vec::new();

    for &parent_id in &parent_nodes {
        let mut cost_left = 110_000_000_u64;
        let r = process_single_spend::<EmptyVisitor>(
            &a,
            &mut ret_a,
            &mut state_a,
            parent_id,
            puzzle_hash,
            amount,
            conds.tree,
            ConsensusFlags::empty(),
            &mut cost_left,
            0,
            &TEST_CONSTANTS,
            None,
        );
        errors_a.push(r.err().map(|e| e.error_code()));
    }

    // Run B: with cache
    let mut ret_b = SpendBundleConditions::default();
    let mut state_b = ParseState::default();
    let mut cache = ConditionsCache::default();
    let mut errors_b: Vec<Option<ErrorCode>> = Vec::new();

    for &parent_id in &parent_nodes {
        let mut cost_left = 110_000_000_u64;
        let r = process_single_spend::<EmptyVisitor>(
            &a,
            &mut ret_b,
            &mut state_b,
            parent_id,
            puzzle_hash,
            amount,
            conds.tree,
            ConsensusFlags::empty(),
            &mut cost_left,
            0,
            &TEST_CONSTANTS,
            Some(&mut cache),
        );
        errors_b.push(r.err().map(|e| e.error_code()));
    }

    assert_eq!(errors_a, errors_b, "per-spend errors differ");

    if errors_a.iter().all(Option::is_none) {
        let validate_a = validate_conditions(&a, &ret_a, &state_a, ConsensusFlags::empty())
            .err()
            .map(|e| e.error_code());
        let validate_b = validate_conditions(&a, &ret_b, &state_b, ConsensusFlags::empty())
            .err()
            .map(|e| e.error_code());
        assert_eq!(
            validate_a, validate_b,
            "validate_conditions results differ between cached and uncached runs"
        );

        let mut owned_a = OwnedSpendBundleConditions::from(&a, ret_a);
        let mut owned_b = OwnedSpendBundleConditions::from(&a, ret_b);
        // HashSet iteration order is non-deterministic; normalize before comparing
        for spend in &mut owned_a.spends {
            spend.create_coin.sort();
        }
        for spend in &mut owned_b.spends {
            spend.create_coin.sort();
        }
        assert_eq!(
            owned_a, owned_b,
            "conditions differ between cached and uncached runs"
        );
    }
});
