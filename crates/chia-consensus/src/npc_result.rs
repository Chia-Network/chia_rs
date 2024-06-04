use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
use crate::gen::condition_tools::u64_to_bytes;
use crate::gen::conditions::{
    process_single_spend, validate_conditions, EmptyVisitor, ParseState, SpendBundleConditions,
};
use crate::gen::flags::MEMPOOL_MODE;
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::run_block_generator::subtract_cost;
use crate::gen::validation_error::{ErrorCode, ValidationErr};
use crate::multiprocess_validation::get_flags_for_height_and_constants;
use chia_protocol::SpendBundle;
use clvm_utils::{tree_hash_cached, TreeHash};
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes;
use std::collections::{HashMap, HashSet};
// #[cfg(feature = "py-bindings")]
// use chia_py_streamable_macro::{PyGetters, PyJsonDict, PyStreamable};
use clvmr::chia_dialect::LIMIT_HEAP;

// we may be able to remove this struct and just return a Rust native Result

// #[cfg_attr(
//     feature = "py-bindings",
//     pyo3::pyclass(module = "chia_rs"),
//     derive(PyJsonDict, PyStreamable, PyGetters),
//     py_uppercase,
//     py_pickle
// )]
// #[streamable]
// pub struct NPCResult {
//     error: Option<u16>,
//     conds: Option<OwnedSpendBundleConditions>,
// }

pub fn get_name_puzzle_conditions(
    spend_bundle: &SpendBundle,
    max_cost: u64,
    mempool_mode: bool,
    height: u32,
    constants: &ConsensusConstants,
) -> Result<OwnedSpendBundleConditions, ValidationErr> {
    let mut flags = get_flags_for_height_and_constants(height, constants);
    if mempool_mode {
        flags |= MEMPOOL_MODE
    };
    // below is an adapted version of the code from run_block_generators::run_block_generator2()
    // it assumes no block references are passed in
    let mut cost_left = max_cost;
    let dialect = ChiaDialect::new(flags);
    let mut a: Allocator = make_allocator(LIMIT_HEAP);
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();
    let mut cache = HashMap::<NodePtr, TreeHash>::new();

    for coin_spend in spend_bundle.coin_spends.clone() {
        // process the spend
        let puz = node_from_bytes(&mut a, coin_spend.puzzle_reveal.as_slice())?;
        let sol = node_from_bytes(&mut a, coin_spend.solution.as_slice())?;
        let parent = a.new_atom(coin_spend.coin.parent_coin_info.as_slice())?;
        let amount = a.new_atom(u64_to_bytes(coin_spend.coin.amount).as_slice())?;
        let Reduction(clvm_cost, conditions) = run_program(&mut a, &dialect, puz, sol, cost_left)?;

        subtract_cost(&a, &mut cost_left, clvm_cost)?;

        let buf = tree_hash_cached(&a, puz, &HashSet::<NodePtr>::new(), &mut cache);
        let puzzle_hash = a.new_atom(&buf)?;

        process_single_spend::<EmptyVisitor>(
            &a,
            &mut ret,
            &mut state,
            parent,
            puzzle_hash,
            amount,
            conditions,
            flags,
            &mut cost_left,
            constants,
        )?;
    }

    validate_conditions(&a, &ret, state, a.nil(), flags)?;

    ret.cost = max_cost - cost_left;
    let Ok(osbc) = OwnedSpendBundleConditions::from(&a, ret) else {
        return Err(ValidationErr(a.nil(), ErrorCode::InvalidSpendBundle));
    };
    Ok(osbc)
}
