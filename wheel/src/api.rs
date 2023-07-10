use crate::compression;
use crate::run_generator::{
    convert_spend_bundle_conds, run_block_generator, run_block_generator2, PySpend,
    PySpendBundleConditions,
};
use chia::allocator::make_allocator;
use chia::gen::flags::{
    AGG_SIG_ARGS, ALLOW_BACKREFS, COND_ARGS_NIL, ENABLE_ASSERT_BEFORE, ENABLE_SOFTFORK_CONDITION,
    LIMIT_ANNOUNCES, LIMIT_OBJECTS, MEMPOOL_MODE, NO_RELATIVE_CONDITIONS_ON_EPHEMERAL,
    NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT,
};
use chia::gen::run_puzzle::run_puzzle as native_run_puzzle;
use chia::gen::solution_generator::solution_generator as native_solution_generator;
use chia::gen::solution_generator::solution_generator_backrefs as native_solution_generator_backrefs;
use chia::merkle_set::compute_merkle_set_root as compute_merkle_root_impl;
use chia_protocol::Bytes32;
use chia_protocol::FullBlock;
use chia_protocol::G1Element;
use chia_protocol::G2Element;
use chia_protocol::{
    ChallengeBlockInfo, ChallengeChainSubSlot, ClassgroupElement, Coin, CoinSpend, CoinState,
    CoinStateUpdate, EndOfSubSlotBundle, Foliage, FoliageTransactionBlock,
    InfusedChallengeChainSubSlot, NewPeakWallet, PoolTarget, Program, ProofOfSpace,
    PuzzleSolutionResponse, RegisterForCoinUpdates, RegisterForPhUpdates, RejectAdditionsRequest,
    RejectBlockHeaders, RejectHeaderBlocks, RejectHeaderRequest, RejectPuzzleSolution,
    RejectRemovalsRequest, RequestAdditions, RequestBlockHeader, RequestBlockHeaders,
    RequestChildren, RequestFeeEstimates, RequestHeaderBlocks, RequestPuzzleSolution,
    RequestRemovals, RequestSesInfo, RespondAdditions, RespondBlockHeader, RespondBlockHeaders,
    RespondChildren, RespondFeeEstimates, RespondHeaderBlocks, RespondPuzzleSolution,
    RespondRemovals, RespondSesInfo, RespondToCoinUpdates, RespondToPhUpdates, RewardChainBlock,
    RewardChainBlockUnfinished, RewardChainSubSlot, SendTransaction, SpendBundle,
    SubEpochChallengeSegment, SubEpochSegments, SubSlotData, SubSlotProofs, TransactionAck,
    TransactionsInfo, VDFInfo, VDFProof,
};
use clvmr::serde::tree_hash_from_stream;
use clvmr::{
    ENABLE_BLS_OPS, ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV, ENABLE_SECP_OPS, LIMIT_HEAP,
    NO_UNKNOWN_OPS,
};
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyBytes;
use pyo3::types::PyModule;
use pyo3::types::PyTuple;
use pyo3::{wrap_pyfunction, PyResult, Python};
use std::convert::TryInto;

use crate::run_program::{run_chia_program, serialized_length};

use crate::adapt_response::eval_err_to_pyresult;
use chia::gen::get_puzzle_and_solution::get_puzzle_and_solution_for_coin as parse_puzzle_solution;
use chia::gen::validation_error::ValidationErr;
use clvmr::allocator::NodePtr;
use clvmr::cost::Cost;
use clvmr::reduction::EvalErr;
use clvmr::reduction::Reduction;
use clvmr::run_program;
use clvmr::serde::node_to_bytes;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs, node_to_bytes_backrefs};
use clvmr::ChiaDialect;

#[pyfunction]
pub fn compute_merkle_set_root<'p>(
    py: Python<'p>,
    values: Vec<&'p PyBytes>,
) -> PyResult<&'p PyBytes> {
    let mut buffer = Vec::<[u8; 32]>::with_capacity(values.len());
    for b in values {
        buffer.push(b.as_bytes().try_into()?);
    }
    Ok(PyBytes::new(py, &compute_merkle_root_impl(&mut buffer)))
}

#[pyfunction]
pub fn tree_hash(py: Python, blob: PyBuffer<u8>) -> PyResult<&PyBytes> {
    if !blob.is_c_contiguous() {
        panic!("tree_hash() must be called with a contiguous buffer");
    }
    let slice =
        unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };
    let mut input = std::io::Cursor::<&[u8]>::new(slice);
    Ok(PyBytes::new(py, &tree_hash_from_stream(&mut input)?))
}

#[pyfunction]
pub fn get_puzzle_and_solution_for_coin<'py>(
    py: Python<'py>,
    program: PyBuffer<u8>,
    args: PyBuffer<u8>,
    max_cost: Cost,
    find_parent: Bytes32,
    find_amount: u64,
    find_ph: Bytes32,
    flags: u32,
) -> PyResult<(&'py PyBytes, &'py PyBytes)> {
    let mut allocator = make_allocator(LIMIT_HEAP);

    if !program.is_c_contiguous() {
        panic!("program must be contiguous");
    }
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    if !args.is_c_contiguous() {
        panic!("args must be contiguous");
    }
    let args = unsafe { std::slice::from_raw_parts(args.buf_ptr() as *const u8, args.len_bytes()) };

    let deserialize = if (flags & ALLOW_BACKREFS) != 0 {
        node_from_bytes_backrefs
    } else {
        node_from_bytes
    };
    let program = deserialize(&mut allocator, program)?;
    let args = deserialize(&mut allocator, args)?;
    let dialect = &ChiaDialect::new(flags);

    let r = py.allow_threads(|| -> Result<(NodePtr, NodePtr), EvalErr> {
        let Reduction(_cost, result) =
            run_program(&mut allocator, dialect, program, args, max_cost)?;
        match parse_puzzle_solution(&allocator, result, find_parent, find_amount, find_ph) {
            Err(ValidationErr(n, _)) => Err(EvalErr(n, "coin not found".to_string())),
            Ok(pair) => Ok(pair),
        }
    });

    let serialize = if (flags & ALLOW_BACKREFS) != 0 {
        node_to_bytes_backrefs
    } else {
        node_to_bytes
    };
    match r {
        Err(eval_err) => eval_err_to_pyresult(py, eval_err, allocator),
        Ok((puzzle, solution)) => Ok((
            PyBytes::new(py, &serialize(&allocator, puzzle)?),
            PyBytes::new(py, &serialize(&allocator, solution)?),
        )),
    }
}

#[pyfunction]
fn run_puzzle(
    puzzle: &[u8],
    solution: &[u8],
    parent_id: &[u8],
    amount: u64,
    max_cost: Cost,
    flags: u32,
) -> PyResult<PySpendBundleConditions> {
    let mut a = make_allocator(LIMIT_HEAP);
    let conds = native_run_puzzle(&mut a, puzzle, solution, parent_id, amount, max_cost, flags)?;
    Ok(convert_spend_bundle_conds(&a, conds))
}

fn convert_list_of_tuples(spends: &PyAny) -> PyResult<Vec<(Coin, &[u8], &[u8])>> {
    let mut native_spends = Vec::<(Coin, &[u8], &[u8])>::new();
    for s in spends.iter()? {
        let tuple = s?.downcast::<PyTuple>()?;
        let coin = tuple.get_item(0)?.extract::<Coin>()?;
        let puzzle = tuple.get_item(1)?.extract::<&[u8]>()?;
        let solution = tuple.get_item(2)?.extract::<&[u8]>()?;
        native_spends.push((coin, puzzle, solution));
    }
    Ok(native_spends)
}

#[pyfunction]
fn solution_generator<'p>(py: Python<'p>, spends: &PyAny) -> PyResult<&'p PyBytes> {
    let spends = convert_list_of_tuples(spends)?;
    Ok(PyBytes::new(py, &native_solution_generator(spends)?))
}

#[pyfunction]
fn solution_generator_backrefs<'p>(py: Python<'p>, spends: &PyAny) -> PyResult<&'p PyBytes> {
    let spends = convert_list_of_tuples(spends)?;
    Ok(PyBytes::new(
        py,
        &native_solution_generator_backrefs(spends)?,
    ))
}

#[pymodule]
pub fn chia_rs(py: Python, m: &PyModule) -> PyResult<()> {
    // generator functions
    m.add_function(wrap_pyfunction!(run_block_generator, m)?)?;
    m.add_function(wrap_pyfunction!(run_block_generator2, m)?)?;
    m.add_function(wrap_pyfunction!(run_puzzle, m)?)?;
    m.add_function(wrap_pyfunction!(solution_generator, m)?)?;
    m.add_function(wrap_pyfunction!(solution_generator_backrefs, m)?)?;
    m.add_class::<PySpendBundleConditions>()?;
    m.add(
        "ELIGIBLE_FOR_DEDUP",
        chia::gen::conditions::ELIGIBLE_FOR_DEDUP,
    )?;
    m.add_class::<PySpend>()?;

    // clvm functions
    m.add("COND_ARGS_NIL", COND_ARGS_NIL)?;
    m.add("NO_UNKNOWN_CONDS", NO_UNKNOWN_CONDS)?;
    m.add("STRICT_ARGS_COUNT", STRICT_ARGS_COUNT)?;
    m.add("LIMIT_ANNOUNCES", LIMIT_ANNOUNCES)?;
    m.add("AGG_SIG_ARGS", AGG_SIG_ARGS)?;
    m.add("ENABLE_ASSERT_BEFORE", ENABLE_ASSERT_BEFORE)?;
    m.add("ENABLE_FIXED_DIV", ENABLE_FIXED_DIV)?;
    m.add("ENABLE_SOFTFORK_CONDITION", ENABLE_SOFTFORK_CONDITION)?;
    m.add(
        "NO_RELATIVE_CONDITIONS_ON_EPHEMERAL",
        NO_RELATIVE_CONDITIONS_ON_EPHEMERAL,
    )?;
    m.add("MEMPOOL_MODE", MEMPOOL_MODE)?;
    m.add("LIMIT_OBJECTS", LIMIT_OBJECTS)?;
    m.add("ALLOW_BACKREFS", ALLOW_BACKREFS)?;

    // Chia classes
    m.add_class::<Coin>()?;
    m.add_class::<G1Element>()?;
    m.add_class::<G2Element>()?;
    m.add_class::<PoolTarget>()?;
    m.add_class::<ClassgroupElement>()?;
    m.add_class::<EndOfSubSlotBundle>()?;
    m.add_class::<TransactionsInfo>()?;
    m.add_class::<FoliageTransactionBlock>()?;
    m.add_class::<Foliage>()?;
    m.add_class::<ProofOfSpace>()?;
    m.add_class::<RewardChainBlockUnfinished>()?;
    m.add_class::<RewardChainBlock>()?;
    m.add_class::<ChallengeBlockInfo>()?;
    m.add_class::<ChallengeChainSubSlot>()?;
    m.add_class::<InfusedChallengeChainSubSlot>()?;
    m.add_class::<RewardChainSubSlot>()?;
    m.add_class::<SubSlotProofs>()?;
    m.add_class::<SpendBundle>()?;
    m.add_class::<Program>()?;
    m.add_class::<CoinSpend>()?;
    m.add_class::<VDFInfo>()?;
    m.add_class::<VDFProof>()?;
    m.add_class::<SubSlotData>()?;
    m.add_class::<SubEpochChallengeSegment>()?;
    m.add_class::<SubEpochSegments>()?;

    // wallet protocol
    m.add_class::<RequestPuzzleSolution>()?;
    m.add_class::<PuzzleSolutionResponse>()?;
    m.add_class::<RespondPuzzleSolution>()?;
    m.add_class::<RejectPuzzleSolution>()?;
    m.add_class::<SendTransaction>()?;
    m.add_class::<TransactionAck>()?;
    m.add_class::<NewPeakWallet>()?;
    m.add_class::<RequestBlockHeader>()?;
    m.add_class::<RespondBlockHeader>()?;
    m.add_class::<RejectHeaderRequest>()?;
    m.add_class::<RequestRemovals>()?;
    m.add_class::<RespondRemovals>()?;
    m.add_class::<RejectRemovalsRequest>()?;
    m.add_class::<RequestAdditions>()?;
    m.add_class::<RespondAdditions>()?;
    m.add_class::<RejectAdditionsRequest>()?;
    m.add_class::<RespondBlockHeaders>()?;
    m.add_class::<RejectBlockHeaders>()?;
    m.add_class::<RequestBlockHeaders>()?;
    m.add_class::<RequestHeaderBlocks>()?;
    m.add_class::<RejectHeaderBlocks>()?;
    m.add_class::<RespondHeaderBlocks>()?;
    m.add_class::<CoinState>()?;
    m.add_class::<RegisterForPhUpdates>()?;
    m.add_class::<RespondToPhUpdates>()?;
    m.add_class::<RegisterForCoinUpdates>()?;
    m.add_class::<RespondToCoinUpdates>()?;
    m.add_class::<CoinStateUpdate>()?;
    m.add_class::<RequestChildren>()?;
    m.add_class::<RespondChildren>()?;
    m.add_class::<RequestSesInfo>()?;
    m.add_class::<RespondSesInfo>()?;
    m.add_class::<RequestFeeEstimates>()?;
    m.add_class::<RespondFeeEstimates>()?;

    m.add_class::<FullBlock>()?;

    // facilities from clvm_rs

    m.add_function(wrap_pyfunction!(run_chia_program, m)?)?;
    m.add("NO_UNKNOWN_OPS", NO_UNKNOWN_OPS)?;
    m.add("LIMIT_HEAP", LIMIT_HEAP)?;
    m.add("ENABLE_BLS_OPS", ENABLE_BLS_OPS)?;
    m.add("ENABLE_SECP_OPS", ENABLE_SECP_OPS)?;
    m.add("ENABLE_BLS_OPS_OUTSIDE_GUARD", ENABLE_BLS_OPS_OUTSIDE_GUARD)?;
    m.add("LIMIT_OBJECTS", LIMIT_OBJECTS)?;

    m.add_function(wrap_pyfunction!(serialized_length, m)?)?;
    m.add_function(wrap_pyfunction!(compute_merkle_set_root, m)?)?;
    m.add_function(wrap_pyfunction!(tree_hash, m)?)?;
    m.add_function(wrap_pyfunction!(get_puzzle_and_solution_for_coin, m)?)?;

    compression::add_submodule(py, m)?;

    Ok(())
}
