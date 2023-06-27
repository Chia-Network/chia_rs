use crate::gen::conditions::{
    parse_conditions, ParseState, Spend, SpendBundleConditions, ELIGIBLE_FOR_DEDUP,
};
use crate::gen::flags::ALLOW_BACKREFS;
use crate::gen::validation_error::ValidationErr;
use chia_protocol::bytes::Bytes32;
use chia_protocol::coin::Coin;
use clvm_utils::tree_hash::tree_hash;
use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};
use std::collections::HashSet;

pub fn run_puzzle(
    a: &mut Allocator,
    puzzle: &[u8],
    solution: &[u8],
    parent_id: &[u8],
    amount: u64,
    max_cost: u64,
    flags: u32,
) -> Result<SpendBundleConditions, ValidationErr> {
    let deserialize = if (flags & ALLOW_BACKREFS) != 0 {
        node_from_bytes_backrefs
    } else {
        node_from_bytes
    };
    let puzzle = deserialize(a, puzzle)?;
    let solution = deserialize(a, solution)?;

    let dialect = ChiaDialect::new(flags);
    let Reduction(clvm_cost, conditions) = run_program(a, &dialect, puzzle, solution, max_cost)?;

    let mut ret = SpendBundleConditions {
        removal_amount: amount as u128,
        ..Default::default()
    };
    let mut state = ParseState::default();

    let puzzle_hash = tree_hash(a, puzzle);
    let coin_id = Coin {
        parent_coin_info: parent_id.into(),
        puzzle_hash: puzzle_hash.into(),
        amount,
    }
    .coin_id()
    .into();

    let spend = Spend {
        parent_id: Bytes32::from(parent_id),
        coin_amount: amount,
        puzzle_hash: Bytes32::from(&puzzle_hash),
        coin_id,
        height_relative: None,
        seconds_relative: None,
        before_height_relative: None,
        before_seconds_relative: None,
        birth_height: None,
        birth_seconds: None,
        create_coin: HashSet::new(),
        agg_sig_me: Vec::new(),
        // assume it's eligible until we see an agg-sig condition
        flags: ELIGIBLE_FOR_DEDUP,
    };

    let mut cost_left = max_cost - clvm_cost;

    parse_conditions(
        a,
        &mut ret,
        &mut state,
        spend,
        conditions,
        flags,
        &mut cost_left,
    )?;
    ret.cost = max_cost - cost_left;
    Ok(ret)
}
