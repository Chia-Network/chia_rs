#![no_main]
use libfuzzer_sys::fuzz_target;

use chia::fuzzing_utils::{make_tree, BitCursor};
use chia::gen::conditions::{parse_conditions, ParseState, Spend, SpendBundleConditions};
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use clvm_utils::tree_hash::tree_hash;
use clvmr::allocator::Allocator;
use std::collections::HashSet;

use chia::gen::flags::{
    COND_ARGS_NIL, ENABLE_ASSERT_BEFORE, ENABLE_SOFTFORK_CONDITION, NO_UNKNOWN_CONDS,
    STRICT_ARGS_COUNT,
};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);

    let mut ret = SpendBundleConditions::default();

    let amount = 1337_u64;
    let parent_id: Bytes32 = b"12345678901234567890123456789012".into();
    let puzzle_hash = tree_hash(&a, input);
    let coin_id = Coin {
        parent_coin_info: parent_id.into(),
        puzzle_hash: puzzle_hash.into(),
        amount,
    }
    .coin_id()
    .into();

    let mut state = ParseState::default();

    for flags in &[
        0,
        ENABLE_ASSERT_BEFORE | COND_ARGS_NIL,
        ENABLE_ASSERT_BEFORE | STRICT_ARGS_COUNT,
        NO_UNKNOWN_CONDS,
        ENABLE_SOFTFORK_CONDITION,
    ] {
        let coin_spend = Spend {
            parent_id,
            coin_amount: amount,
            puzzle_hash: puzzle_hash.into(),
            coin_id,
            height_relative: None,
            seconds_relative: None,
            before_height_relative: None,
            before_seconds_relative: None,
            birth_height: None,
            birth_seconds: None,
            create_coin: HashSet::new(),
            agg_sig_me: Vec::new(),
            flags: 0_u32,
        };
        let mut max_cost: u64 = 3300000000;
        let _ret = parse_conditions(
            &a,
            &mut ret,
            &mut state,
            coin_spend,
            input,
            *flags,
            &mut max_cost,
        );
    }
});
