use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
use crate::gen::conditions::{process_single_spend, validate_conditions, EmptyVisitor, ParseState, SpendBundleConditions};
use clvm_utils::{tree_hash_cached, TreeHash};
use clvmr::run_program::run_program;
use clvmr::reduction::Reduction;
use crate::gen::flags::MEMPOOL_MODE;
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::run_block_generator::{extract_n, subtract_cost};
use std::collections::{HashMap, HashSet};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::serde::node_from_bytes;
use clvmr::allocator::{Allocator, NodePtr};
use crate::gen::validation_error::{first, ErrorCode, ValidationErr};
use crate::multiprocess_validation::get_flags_for_height_and_constants;
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
    generator_program: Vec<u8>,
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
    let program = node_from_bytes(&mut a, generator_program.as_slice())?;
    let env = a.nil();
    let Reduction(clvm_cost, mut all_spends) = run_program(&mut a, &dialect, program, env, cost_left)?;

    subtract_cost(&a, &mut cost_left, clvm_cost)?;
    all_spends = first(&a, all_spends)?;
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();
    let mut cache = HashMap::<NodePtr, TreeHash>::new();

    while let Some((spend, rest)) = a.next(all_spends) {
        all_spends = rest;
        // process the spend
        let [parent_id, puzzle, amount, solution, _spend_level_extra] =
            extract_n::<5>(&a, spend, ErrorCode::InvalidCondition)?;

        let Reduction(clvm_cost, conditions) =
            run_program(&mut a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(&a, &mut cost_left, clvm_cost)?;

        let buf = tree_hash_cached(&a, puzzle, &HashSet::<NodePtr>::new(), &mut cache);
        let puzzle_hash = a.new_atom(&buf)?;

        process_single_spend::<EmptyVisitor>(
            &a,
            &mut ret,
            &mut state,
            parent_id,
            puzzle_hash,
            amount,
            conditions,
            flags,
            &mut cost_left,
        )?;
    }
    if a.atom_len(all_spends) != 0 {
        return Err(ValidationErr(all_spends, ErrorCode::GeneratorRuntimeError));
    }

    validate_conditions(&a, &ret, state, a.nil(), flags)?;

    ret.cost = max_cost - cost_left;
    let Ok(osbc) = OwnedSpendBundleConditions::from(&a, ret) else {return Err(ValidationErr(all_spends, ErrorCode::InvalidSpendBundle))};
    Ok(osbc)
}
