use crate::compression;
use crate::run_generator::{PySpend, PySpendBundleConditions, convert_spend_bundle_conds, __pyo3_get_function_run_generator, __pyo3_get_function_run_block_generator};
use chia::gen::run_puzzle::run_puzzle as native_run_puzzle;
use chia::gen::flags::COND_ARGS_NIL;
use chia::gen::flags::NO_UNKNOWN_CONDS;
use chia::gen::flags::STRICT_ARGS_COUNT;
use chia::gen::flags::MEMPOOL_MODE;
use chia::gen::flags::ENABLE_ASSERT_BEFORE;
use chia::merkle_set::compute_merkle_set_root as compute_merkle_root_impl;
use chia::allocator::make_allocator;
use chia_protocol::Bytes32;
use chia_protocol::G1Element;
use chia_protocol::G2Element;
use chia_protocol::FullBlock;
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
    TransactionsInfo, VDFInfo, VDFProof
};
use std::convert::TryInto;
use clvmr::LIMIT_HEAP;
use clvmr::LIMIT_STACK;
use clvmr::NO_UNKNOWN_OPS;
use clvmr::serde::tree_hash_from_stream;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyModule;
use pyo3::{wrap_pyfunction, PyResult, Python};

use crate::run_program::{
    __pyo3_get_function_run_chia_program, __pyo3_get_function_serialized_length,
};

use crate::adapt_response::eval_err_to_pyresult;
use chia::gen::get_puzzle_and_solution::get_puzzle_and_solution_for_coin as parse_puzzle_solution;
use chia::gen::validation_error::ValidationErr;
use clvmr::allocator::NodePtr;
use clvmr::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::node::Node;
use clvmr::reduction::EvalErr;
use clvmr::reduction::Reduction;
use clvmr::run_program;
use clvmr::serde::node_from_bytes;
use clvmr::serde::node_to_bytes;

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
pub fn tree_hash(py: Python, blob: pyo3::buffer::PyBuffer<u8>) -> PyResult<&PyBytes> {
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
    program: &[u8],
    args: &[u8],
    max_cost: Cost,
    find_parent: Bytes32,
    find_amount: u64,
    find_ph: Bytes32,
) -> PyResult<(&'py PyBytes, &'py PyBytes)> {
    let mut allocator = make_allocator(LIMIT_HEAP);
    let program = node_from_bytes(&mut allocator, program)?;
    let args = node_from_bytes(&mut allocator, args)?;
    let dialect = &ChiaDialect::new(LIMIT_STACK);

    let r = py.allow_threads(|| -> Result<(NodePtr, NodePtr), EvalErr> {
        let Reduction(_cost, result) =
            run_program(&mut allocator, dialect, program, args, max_cost)?;
        match parse_puzzle_solution(&allocator, result, find_parent, find_amount, find_ph) {
            Err(ValidationErr(n, _)) => Err(EvalErr(n, "coin not found".to_string())),
            Ok(pair) => Ok(pair),
        }
    });

    match r {
        Err(eval_err) => eval_err_to_pyresult(py, eval_err, allocator),
        Ok((puzzle, solution)) => {
            Ok((
                PyBytes::new(py, &node_to_bytes(&Node::new(&allocator, puzzle))?),
                PyBytes::new(py, &node_to_bytes(&Node::new(&allocator, solution))?)
            ))
        },
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
    Ok(convert_spend_bundle_conds(&mut a, conds))
}

#[pymodule]
pub fn chia_rs(py: Python, m: &PyModule) -> PyResult<()> {
    // generator functions
    m.add_function(wrap_pyfunction!(run_generator, m)?)?;
    m.add_function(wrap_pyfunction!(run_block_generator, m)?)?;
    m.add_function(wrap_pyfunction!(run_puzzle, m)?)?;
    m.add_class::<PySpendBundleConditions>()?;
    m.add("ELIGIBLE_FOR_DEDUP", chia::gen::conditions::ELIGIBLE_FOR_DEDUP)?;
    m.add_class::<PySpend>()?;

    // clvm functions
    m.add("COND_ARGS_NIL", COND_ARGS_NIL)?;
    m.add("NO_UNKNOWN_CONDS", NO_UNKNOWN_CONDS)?;
    m.add("STRICT_ARGS_COUNT", STRICT_ARGS_COUNT)?;
    m.add("ENABLE_ASSERT_BEFORE", ENABLE_ASSERT_BEFORE)?;
    m.add("MEMPOOL_MODE", MEMPOOL_MODE)?;

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
    m.add("LIMIT_STACK", LIMIT_STACK)?;

    m.add_function(wrap_pyfunction!(serialized_length, m)?)?;
    m.add_function(wrap_pyfunction!(compute_merkle_set_root, m)?)?;
    m.add_function(wrap_pyfunction!(tree_hash, m)?)?;
    m.add_function(wrap_pyfunction!(get_puzzle_and_solution_for_coin, m)?)?;

    compression::add_submodule(py, m)?;

    Ok(())
}
