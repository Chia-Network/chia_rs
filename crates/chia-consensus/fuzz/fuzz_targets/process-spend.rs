#![no_main]
use chia_consensus::conditions::{
    MempoolVisitor, ParseState, SpendBundleConditions, process_single_spend,
};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::{NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT};
use clvm_fuzzing::ArbitraryClvmTree;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|args: (ArbitraryClvmTree, [u8; 32], [u8; 32], u64)| {
    let (conds, parent_id, puzzle_hash, amount) = args;
    let mut a = conds.allocator;
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    let parent_id = a.new_atom(&parent_id).expect("new_atom");
    let puzzle_hash = a.new_atom(&puzzle_hash).expect("new_atom");
    let amount = a.new_number(amount.into()).expect("new_atom");

    for flags in &[0, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS] {
        let mut cost_left = 110_000_000;
        let _ = process_single_spend::<MempoolVisitor>(
            &a,
            &mut ret,
            &mut state,
            parent_id,
            puzzle_hash,
            amount,
            conds.tree,
            *flags,
            &mut cost_left,
            0, // clvm_cost
            &TEST_CONSTANTS,
        );
    }
});
