#![no_main]
use chia_consensus::gen::conditions::{
    process_single_spend, MempoolVisitor, ParseState, SpendBundleConditions,
};
use chia_consensus::gen::flags::{COND_ARGS_NIL, NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT};
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_tree, BitCursor};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    let parent_id = a.new_atom(&[0_u8; 32]).unwrap();
    let puzzle_hash = a.new_atom(&[0_u8; 32]).unwrap();
    let amount = a.new_atom(&[100_u8]).unwrap();

    let conds = make_tree(&mut a, &mut BitCursor::new(data), false);

    for flags in &[0, COND_ARGS_NIL, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS] {
        let mut cost_left = 11000000;
        let _ = process_single_spend::<MempoolVisitor>(
            &a,
            &mut ret,
            &mut state,
            parent_id,
            puzzle_hash,
            amount,
            conds,
            *flags,
            &mut cost_left,
        );
    }
});
