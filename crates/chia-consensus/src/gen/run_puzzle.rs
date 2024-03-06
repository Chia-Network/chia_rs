use crate::gen::conditions::{parse_conditions, ParseState, Spend, SpendBundleConditions};
use crate::gen::flags::ALLOW_BACKREFS;
use crate::gen::spend_visitor::SpendVisitor;
use crate::gen::validation_error::ValidationErr;
use chia_protocol::bytes::Bytes32;
use chia_protocol::coin::Coin;
use clvm_utils::tree_hash;
use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};
use std::sync::Arc;

pub fn run_puzzle<V: SpendVisitor>(
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
    let coin_id = Arc::<Bytes32>::new(
        Coin {
            parent_coin_info: parent_id.try_into().unwrap(),
            puzzle_hash: puzzle_hash.into(),
            amount,
        }
        .coin_id()
        .into(),
    );

    let mut spend = Spend::new(
        a.new_atom(parent_id)?,
        amount,
        a.new_atom(&puzzle_hash)?,
        coin_id,
    );

    let mut visitor = V::new_spend(&mut spend);

    let mut cost_left = max_cost - clvm_cost;

    parse_conditions(
        a,
        &mut ret,
        &mut state,
        spend,
        conditions,
        flags,
        &mut cost_left,
        &mut visitor,
    )?;
    ret.cost = max_cost - cost_left;
    Ok(ret)
}
