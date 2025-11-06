#![no_main]
use libfuzzer_sys::{arbitrary, fuzz_target};

use chia_consensus::conditions::{
    parse_conditions, MempoolVisitor, ParseState, SpendBundleConditions, SpendConditions,
};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::spend_visitor::SpendVisitor;
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use clvm_fuzzing::make_list;
use clvm_utils::tree_hash;
use clvmr::{Allocator, NodePtr};
use std::collections::HashSet;

use chia_consensus::flags::{NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let mut unstructured = arbitrary::Unstructured::new(data);
    let input = make_list(&mut a, &mut unstructured);
    // conditions are a list of lists
    let input = a.new_pair(input, NodePtr::NIL).unwrap();

    let mut ret = SpendBundleConditions::default();

    let amount = 1337_u64;
    let parent_id: Bytes32 = b"12345678901234567890123456789012".into();
    let puzzle_hash = tree_hash(&a, input);
    let coin_id = Coin::new(parent_id, puzzle_hash.into(), amount).coin_id();
    let parent_id = a.new_atom(&parent_id).expect("atom failed");
    let puzzle_hash = a.new_atom(&puzzle_hash).expect("atom failed");

    let mut state = ParseState::default();

    for flags in &[0, STRICT_ARGS_COUNT, NO_UNKNOWN_CONDS] {
        let mut coin_spend = SpendConditions {
            parent_id,
            coin_amount: amount,
            puzzle_hash,
            coin_id,
            height_relative: None,
            seconds_relative: None,
            before_height_relative: None,
            before_seconds_relative: None,
            birth_height: None,
            birth_seconds: None,
            create_coin: HashSet::new(),
            agg_sig_me: Vec::new(),
            agg_sig_parent: Vec::new(),
            agg_sig_puzzle: Vec::new(),
            agg_sig_amount: Vec::new(),
            agg_sig_puzzle_amount: Vec::new(),
            agg_sig_parent_amount: Vec::new(),
            agg_sig_parent_puzzle: Vec::new(),
            flags: 0_u32,
            execution_cost: 0_u64,
            condition_cost: 0_u64,
            fingerprint: [0; 32],
        };
        let mut visitor = MempoolVisitor::new_spend(&mut coin_spend);
        let mut max_cost = 3_300_000_000;
        let _ret = parse_conditions(
            &a,
            &mut ret,
            &mut state,
            coin_spend,
            input,
            *flags,
            &mut max_cost,
            &TEST_CONSTANTS,
            &mut visitor,
        );
    }
});
