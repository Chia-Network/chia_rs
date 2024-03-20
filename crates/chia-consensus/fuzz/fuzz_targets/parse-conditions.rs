#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::gen::conditions::{
    parse_conditions, MempoolVisitor, ParseState, Spend, SpendBundleConditions,
};
use chia_consensus::gen::spend_visitor::SpendVisitor;
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use clvm_utils::tree_hash;
use clvmr::{Allocator, NodePtr};
use fuzzing_utils::{make_list, BitCursor};
use std::collections::HashSet;
use std::sync::Arc;

use chia_consensus::gen::flags::{
    COND_ARGS_NIL, ENABLE_MESSAGE_CONDITIONS, ENABLE_SOFTFORK_CONDITION, NO_UNKNOWN_CONDS,
    STRICT_ARGS_COUNT,
};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_list(&mut a, &mut BitCursor::new(data));
    // conditions are a list of lists
    let input = a.new_pair(input, NodePtr::NIL).unwrap();

    let mut ret = SpendBundleConditions::default();

    let amount = 1337_u64;
    let parent_id: Bytes32 = b"12345678901234567890123456789012".into();
    let puzzle_hash = tree_hash(&a, input);
    let coin_id = Arc::<Bytes32>::new(
        Coin {
            parent_coin_info: parent_id,
            puzzle_hash: puzzle_hash.into(),
            amount,
        }
        .coin_id(),
    );
    let parent_id = a.new_atom(&parent_id).expect("atom failed");
    let puzzle_hash = a.new_atom(&puzzle_hash).expect("atom failed");

    let mut state = ParseState::default();

    for flags in &[
        0,
        COND_ARGS_NIL,
        STRICT_ARGS_COUNT,
        NO_UNKNOWN_CONDS,
        ENABLE_SOFTFORK_CONDITION,
        ENABLE_MESSAGE_CONDITIONS,
    ] {
        let mut coin_spend = Spend {
            parent_id,
            coin_amount: amount,
            puzzle_hash,
            coin_id: coin_id.clone(),
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
        };
        let mut visitor = MempoolVisitor::new_spend(&mut coin_spend);
        let mut max_cost: u64 = 3300000000;
        let _ret = parse_conditions(
            &a,
            &mut ret,
            &mut state,
            coin_spend,
            input,
            *flags,
            &mut max_cost,
            &mut visitor,
        );
    }
});
