use crate::run_generator::{run_block_generator, run_block_generator2};
use crate::visitor::Visitor;
use aug_scheme_mpl::AugSchemeMPL;
use chia_consensus::allocator::make_allocator;
use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::gen::conditions::{MempoolVisitor, ELIGIBLE_FOR_DEDUP, ELIGIBLE_FOR_FF};
use chia_consensus::gen::flags::{
    AGG_SIG_ARGS, ALLOW_BACKREFS, ANALYZE_SPENDS, COND_ARGS_NIL, DISALLOW_INFINITY_G1,
    ENABLE_MESSAGE_CONDITIONS, ENABLE_SOFTFORK_CONDITION, MEMPOOL_MODE, NO_UNKNOWN_CONDS,
    STRICT_ARGS_COUNT,
};
use chia_consensus::gen::owned_conditions::{OwnedSpend, OwnedSpendBundleConditions};
use chia_consensus::gen::run_puzzle::run_puzzle as native_run_puzzle;
use chia_consensus::gen::solution_generator::solution_generator as native_solution_generator;
use chia_consensus::gen::solution_generator::solution_generator_backrefs as native_solution_generator_backrefs;
use chia_consensus::merkle_set::compute_merkle_set_root as compute_merkle_root_impl;
use chia_consensus::merkle_tree::{validate_merkle_proof, MerkleSet};
use chia_protocol::{
    BlockRecord, Bytes32, ChallengeBlockInfo, ChallengeChainSubSlot, ClassgroupElement, Coin,
    CoinSpend, CoinState, CoinStateFilters, CoinStateUpdate, EndOfSubSlotBundle, Foliage,
    FoliageBlockData, FoliageTransactionBlock, FullBlock, Handshake, HeaderBlock,
    InfusedChallengeChainSubSlot, LazyNode, Message, NewCompactVDF, NewPeak, NewPeakWallet,
    NewSignagePointOrEndOfSubSlot, NewTransaction, NewUnfinishedBlock, NewUnfinishedBlock2,
    PoolTarget, Program, ProofBlockHeader, ProofOfSpace, PuzzleSolutionResponse, RecentChainData,
    RegisterForCoinUpdates, RegisterForPhUpdates, RejectAdditionsRequest, RejectBlock,
    RejectBlockHeaders, RejectBlocks, RejectCoinState, RejectHeaderBlocks, RejectHeaderRequest,
    RejectPuzzleSolution, RejectPuzzleState, RejectRemovalsRequest, RequestAdditions, RequestBlock,
    RequestBlockHeader, RequestBlockHeaders, RequestBlocks, RequestChildren, RequestCoinState,
    RequestCompactVDF, RequestFeeEstimates, RequestHeaderBlocks, RequestMempoolTransactions,
    RequestPeers, RequestProofOfWeight, RequestPuzzleSolution, RequestPuzzleState, RequestRemovals,
    RequestRemoveCoinSubscriptions, RequestRemovePuzzleSubscriptions, RequestSesInfo,
    RequestSignagePointOrEndOfSubSlot, RequestTransaction, RequestUnfinishedBlock,
    RequestUnfinishedBlock2, RespondAdditions, RespondBlock, RespondBlockHeader,
    RespondBlockHeaders, RespondBlocks, RespondChildren, RespondCoinState, RespondCompactVDF,
    RespondEndOfSubSlot, RespondFeeEstimates, RespondHeaderBlocks, RespondPeers,
    RespondProofOfWeight, RespondPuzzleSolution, RespondPuzzleState, RespondRemovals,
    RespondRemoveCoinSubscriptions, RespondRemovePuzzleSubscriptions, RespondSesInfo,
    RespondSignagePoint, RespondToCoinUpdates, RespondToPhUpdates, RespondTransaction,
    RespondUnfinishedBlock, RewardChainBlock, RewardChainBlockUnfinished, RewardChainSubSlot,
    SendTransaction, SpendBundle, SubEpochChallengeSegment, SubEpochData, SubEpochSegments,
    SubEpochSummary, SubSlotData, SubSlotProofs, TimestampedPeerInfo, TransactionAck,
    TransactionsInfo, UnfinishedBlock, UnfinishedHeaderBlock, VDFInfo, VDFProof, WeightProof,
};
use chia_traits::{Bytes, Int, ReadableBuffer};
use clvm_utils::tree_hash_from_bytes;
use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV, LIMIT_HEAP, NO_UNKNOWN_OPS};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::PyBytes;
use pyo3::types::PyTuple;
use pyo3::wrap_pyfunction;

use crate::run_program::{run_chia_program, serialized_length};

use crate::adapt_response::eval_err_to_pyresult;
use chia_consensus::fast_forward::fast_forward_singleton as native_ff;
use chia_consensus::gen::get_puzzle_and_solution::get_puzzle_and_solution_for_coin as parse_puzzle_solution;
use chia_consensus::gen::validation_error::ValidationErr;
use clvmr::allocator::NodePtr;
use clvmr::cost::Cost;
use clvmr::reduction::EvalErr;
use clvmr::reduction::Reduction;
use clvmr::run_program;
use clvmr::serde::node_to_bytes;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};
use clvmr::ChiaDialect;

use chia_bls::{BlsCache, GTElement, PublicKey, SecretKey, Signature};

mod aug_scheme_mpl;

#[pyfunction]
pub fn compute_merkle_set_root<'p>(
    py: Python<'p>,
    values: Vec<&'p PyBytes>,
) -> PyResult<Bound<'p, PyBytes>> {
    let mut buffer = Vec::<[u8; 32]>::with_capacity(values.len());
    for b in values {
        buffer.push(b.as_bytes().try_into()?);
    }
    Ok(PyBytes::new_bound(
        py,
        &compute_merkle_root_impl(&mut buffer),
    ))
}

#[pyfunction]
pub fn confirm_included_already_hashed(
    root: Bytes32,
    item: Bytes32,
    proof: &[u8],
) -> PyResult<bool> {
    validate_merkle_proof(proof, (&item).into(), (&root).into())
        .map_err(|_| PyValueError::new_err("Invalid proof"))
}

#[pyfunction]
pub fn confirm_not_included_already_hashed(
    root: Bytes32,
    item: Bytes32,
    proof: &[u8],
) -> PyResult<bool> {
    validate_merkle_proof(proof, (&item).into(), (&root).into())
        .map_err(|_| PyValueError::new_err("Invalid proof"))
        .map(|r| !r)
}

#[pyfunction]
pub fn tree_hash(py: Python<'_>, blob: PyBuffer<u8>) -> PyResult<Bound<'_, PyBytes>> {
    assert!(
        blob.is_c_contiguous(),
        "tree_hash() must be called with a contiguous buffer"
    );
    let slice =
        unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };
    Ok(PyBytes::new_bound(py, &tree_hash_from_bytes(slice)?))
}

#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub fn get_puzzle_and_solution_for_coin(
    py: Python<'_>,
    program: PyBuffer<u8>,
    args: PyBuffer<u8>,
    max_cost: Cost,
    find_parent: Bytes32,
    find_amount: u64,
    find_ph: Bytes32,
    flags: u32,
) -> PyResult<(Bound<'_, PyBytes>, Bound<'_, PyBytes>)> {
    let mut allocator = make_allocator(LIMIT_HEAP);

    assert!(program.is_c_contiguous(), "program must be contiguous");
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    assert!(args.is_c_contiguous(), "args must be contiguous");
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

    // keep serializing normally, until wallets support backrefs
    let serialize = node_to_bytes;
    /*
        let serialize = if (flags & ALLOW_BACKREFS) != 0 {
            node_to_bytes_backrefs
        } else {
            node_to_bytes
        };
    */
    match r {
        Err(eval_err) => eval_err_to_pyresult(eval_err, &allocator),
        Ok((puzzle, solution)) => Ok((
            PyBytes::new_bound(py, &serialize(&allocator, puzzle)?),
            PyBytes::new_bound(py, &serialize(&allocator, solution)?),
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
    constants: &ConsensusConstants,
) -> PyResult<OwnedSpendBundleConditions> {
    let mut a = make_allocator(LIMIT_HEAP);
    let conds = native_run_puzzle::<MempoolVisitor>(
        &mut a, puzzle, solution, parent_id, amount, max_cost, flags, constants,
    )?;
    Ok(OwnedSpendBundleConditions::from(&a, conds))
}

// this is like a CoinSpend but with references to the puzzle and solution,
// rather than owning them
type CoinSpendRef = (Coin, PyBackedBytes, PyBackedBytes);

fn convert_list_of_tuples(spends: &Bound<'_, PyAny>) -> PyResult<Vec<CoinSpendRef>> {
    let mut native_spends = Vec::<CoinSpendRef>::new();
    for s in spends.iter()? {
        let s = s?;
        let tuple = s.downcast::<PyTuple>()?;
        let coin = tuple.get_item(0)?.extract::<Coin>()?;
        let puzzle = tuple.get_item(1)?.extract::<PyBackedBytes>()?;
        let solution = tuple.get_item(2)?.extract::<PyBackedBytes>()?;
        native_spends.push((coin, puzzle, solution));
    }
    Ok(native_spends)
}

#[pyfunction]
fn solution_generator<'p>(
    py: Python<'p>,
    spends: &Bound<'_, PyAny>,
) -> PyResult<Bound<'p, PyBytes>> {
    let spends = convert_list_of_tuples(spends)?;
    Ok(PyBytes::new_bound(py, &native_solution_generator(spends)?))
}

#[pyfunction]
fn solution_generator_backrefs<'p>(
    py: Python<'p>,
    spends: &Bound<'_, PyAny>,
) -> PyResult<Bound<'p, PyBytes>> {
    let spends = convert_list_of_tuples(spends)?;
    Ok(PyBytes::new_bound(
        py,
        &native_solution_generator_backrefs(spends)?,
    ))
}

#[pyfunction]
fn supports_fast_forward(spend: &CoinSpend) -> bool {
    // the test function just attempts the rebase onto a dummy parent coin
    let new_parent = Coin {
        parent_coin_info: [0_u8; 32].into(),
        puzzle_hash: spend.coin.puzzle_hash,
        amount: spend.coin.amount,
    };
    let new_coin = Coin {
        parent_coin_info: new_parent.coin_id(),
        puzzle_hash: spend.coin.puzzle_hash,
        amount: spend.coin.amount,
    };

    let mut a = make_allocator(LIMIT_HEAP);
    let Ok(puzzle) = node_from_bytes(&mut a, spend.puzzle_reveal.as_slice()) else {
        return false;
    };
    let Ok(solution) = node_from_bytes(&mut a, spend.solution.as_slice()) else {
        return false;
    };

    native_ff(
        &mut a,
        puzzle,
        solution,
        &spend.coin,
        &new_coin,
        &new_parent,
    )
    .is_ok()
}

#[pyfunction]
fn fast_forward_singleton<'p>(
    py: Python<'p>,
    spend: &CoinSpend,
    new_coin: &Coin,
    new_parent: &Coin,
) -> PyResult<Bound<'p, PyBytes>> {
    let mut a = make_allocator(LIMIT_HEAP);
    let puzzle = node_from_bytes(&mut a, spend.puzzle_reveal.as_slice())?;
    let solution = node_from_bytes(&mut a, spend.solution.as_slice())?;

    let new_solution = native_ff(&mut a, puzzle, solution, &spend.coin, new_coin, new_parent)?;
    Ok(PyBytes::new_bound(
        py,
        node_to_bytes(&a, new_solution)?.as_slice(),
    ))
}

#[pymodule]
#[allow(clippy::unnecessary_wraps)]
pub fn chia_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    bindings(m);
    Ok(())
}

pub fn bindings(m: &impl Visitor) {
    // clvmr constants
    m.int("NO_UNKNOWN_OPS", NO_UNKNOWN_OPS);
    m.int("LIMIT_HEAP", LIMIT_HEAP);
    m.int("ENABLE_BLS_OPS_OUTSIDE_GUARD", ENABLE_BLS_OPS_OUTSIDE_GUARD);

    // chia-consensus constants
    m.int("COND_ARGS_NIL", COND_ARGS_NIL);
    m.int("NO_UNKNOWN_CONDS", NO_UNKNOWN_CONDS);
    m.int("STRICT_ARGS_COUNT", STRICT_ARGS_COUNT);
    m.int("AGG_SIG_ARGS", AGG_SIG_ARGS);
    m.int("ENABLE_FIXED_DIV", ENABLE_FIXED_DIV);
    m.int("ENABLE_SOFTFORK_CONDITION", ENABLE_SOFTFORK_CONDITION);
    m.int("ENABLE_MESSAGE_CONDITIONS", ENABLE_MESSAGE_CONDITIONS);
    m.int("MEMPOOL_MODE", MEMPOOL_MODE);
    m.int("ALLOW_BACKREFS", ALLOW_BACKREFS);
    m.int("ANALYZE_SPENDS", ANALYZE_SPENDS);
    m.int("DISALLOW_INFINITY_G1", DISALLOW_INFINITY_G1);
    m.int("ELIGIBLE_FOR_DEDUP", ELIGIBLE_FOR_DEDUP);
    m.int("ELIGIBLE_FOR_FF", ELIGIBLE_FOR_FF);

    // generator functions
    m.function::<(Option<u32>, Option<OwnedSpendBundleConditions>)>(
        "run_block_generator",
        |m| m.add_function(wrap_pyfunction!(run_block_generator, m)?),
        |m| {
            m.param::<ReadableBuffer>("program")
                .param::<Vec<ReadableBuffer>>("args")
                .param::<Int>("max_cost")
                .param::<ConsensusConstants>("constants")
        },
    );

    m.function::<(Option<u32>, Option<OwnedSpendBundleConditions>)>(
        "run_block_generator2",
        |m| m.add_function(wrap_pyfunction!(run_block_generator2, m)?),
        |m| {
            m.param::<ReadableBuffer>("program")
                .param::<Vec<ReadableBuffer>>("args")
                .param::<Int>("max_cost")
                .param::<ConsensusConstants>("constants")
        },
    );

    m.function::<OwnedSpendBundleConditions>(
        "run_puzzle",
        |m| m.add_function(wrap_pyfunction!(run_puzzle, m)?),
        |m| {
            m.param::<Bytes>("puzzle")
                .param::<Bytes>("solution")
                .param::<Bytes32>("parent_id")
                .param::<Int>("amount")
                .param::<Int>("max_cost")
                .param::<Int>("flags")
                .param::<ConsensusConstants>("constants")
        },
    );

    m.function::<Bytes>(
        "solution_generator",
        |m| m.add_function(wrap_pyfunction!(solution_generator, m)?),
        |m| m.param::<Vec<(Coin, Bytes, Bytes)>>("spends"),
    );

    m.function::<Bytes>(
        "solution_generator_backrefs",
        |m| m.add_function(wrap_pyfunction!(solution_generator_backrefs, m)?),
        |m| m.param::<Vec<(Coin, Bytes, Bytes)>>("spends"),
    );

    m.function::<bool>(
        "supports_fast_forward",
        |m| m.add_function(wrap_pyfunction!(supports_fast_forward, m)?),
        |m| m.param::<CoinSpend>("spend"),
    );

    m.function::<Bytes>(
        "fast_forward_singleton",
        |m| m.add_function(wrap_pyfunction!(fast_forward_singleton, m)?),
        |m| {
            m.param::<CoinSpend>("spend")
                .param::<Coin>("new_coin")
                .param::<Coin>("new_parent")
        },
    );

    // merkle tree functions
    m.function::<Bytes>(
        "confirm_included_already_hashed",
        |m| m.add_function(wrap_pyfunction!(confirm_included_already_hashed, m)?),
        |m| {
            m.param::<CoinSpend>("spend")
                .param::<Bytes32>("root")
                .param::<Bytes32>("item")
                .param::<Bytes>("proof")
        },
    );

    m.function::<Bytes>(
        "confirm_not_included_already_hashed",
        |m| m.add_function(wrap_pyfunction!(confirm_not_included_already_hashed, m)?),
        |m| {
            m.param::<CoinSpend>("spend")
                .param::<Bytes32>("root")
                .param::<Bytes32>("item")
                .param::<Bytes>("proof")
        },
    );

    // clvmr functions
    m.function::<Bytes>(
        "compute_merkle_set_root",
        |m| m.add_function(wrap_pyfunction!(compute_merkle_set_root, m)?),
        |m| m.param::<Vec<Bytes>>("values"),
    );

    m.function::<(Int, LazyNode)>(
        "run_chia_program",
        |m| m.add_function(wrap_pyfunction!(run_chia_program, m)?),
        |m| {
            m.param::<Bytes>("program")
                .param::<Bytes>("args")
                .param::<Int>("max_cost")
                .param::<Int>("flags")
        },
    );

    m.function::<Int>(
        "serialized_length",
        |m| m.add_function(wrap_pyfunction!(serialized_length, m)?),
        |m| m.param::<ReadableBuffer>("program"),
    );

    m.function::<Bytes32>(
        "tree_hash",
        |m| m.add_function(wrap_pyfunction!(tree_hash, m)?),
        |m| m.param::<ReadableBuffer>("program"),
    );

    m.function::<(Bytes, Bytes)>(
        "get_puzzle_and_solution_for_coin",
        |m| m.add_function(wrap_pyfunction!(get_puzzle_and_solution_for_coin, m)?),
        |m| {
            m.param::<ReadableBuffer>("program")
                .param::<ReadableBuffer>("args")
                .param::<Int>("max_cost")
                .param::<Bytes32>("find_parent")
                .param::<Int>("find_amount")
                .param::<Bytes32>("find_ph")
                .param::<Int>("flags")
        },
    );

    // chia-consensus
    m.visit::<OwnedSpendBundleConditions>();
    m.visit::<OwnedSpend>();
    m.visit::<ConsensusConstants>();
    m.visit::<MerkleSet>();

    // chia-protocol
    m.visit::<Message>();
    m.visit::<Handshake>();
    m.visit::<Coin>();
    m.visit::<PoolTarget>();
    m.visit::<ClassgroupElement>();
    m.visit::<EndOfSubSlotBundle>();
    m.visit::<TransactionsInfo>();
    m.visit::<FoliageTransactionBlock>();
    m.visit::<FoliageBlockData>();
    m.visit::<Foliage>();
    m.visit::<ProofOfSpace>();
    m.visit::<RewardChainBlockUnfinished>();
    m.visit::<RewardChainBlock>();
    m.visit::<ChallengeBlockInfo>();
    m.visit::<ChallengeChainSubSlot>();
    m.visit::<InfusedChallengeChainSubSlot>();
    m.visit::<RewardChainSubSlot>();
    m.visit::<SubSlotProofs>();
    m.visit::<SpendBundle>();
    m.visit::<Program>();
    m.visit::<CoinSpend>();
    m.visit::<VDFInfo>();
    m.visit::<VDFProof>();
    m.visit::<SubSlotData>();
    m.visit::<SubEpochData>();
    m.visit::<SubEpochChallengeSegment>();
    m.visit::<SubEpochSegments>();
    m.visit::<SubEpochSummary>();
    m.visit::<UnfinishedBlock>();
    m.visit::<FullBlock>();
    m.visit::<BlockRecord>();
    m.visit::<WeightProof>();
    m.visit::<RecentChainData>();
    m.visit::<ProofBlockHeader>();
    m.visit::<TimestampedPeerInfo>();
    m.visit::<LazyNode>();

    // chia-protocol (wallet)
    m.visit::<RequestPuzzleSolution>();
    m.visit::<PuzzleSolutionResponse>();
    m.visit::<RespondPuzzleSolution>();
    m.visit::<RejectPuzzleSolution>();
    m.visit::<SendTransaction>();
    m.visit::<TransactionAck>();
    m.visit::<NewPeakWallet>();
    m.visit::<RequestBlockHeader>();
    m.visit::<RespondBlockHeader>();
    m.visit::<RejectHeaderRequest>();
    m.visit::<RequestRemovals>();
    m.visit::<RespondRemovals>();
    m.visit::<RejectRemovalsRequest>();
    m.visit::<RequestAdditions>();
    m.visit::<RespondAdditions>();
    m.visit::<RejectAdditionsRequest>();
    m.visit::<RespondBlockHeaders>();
    m.visit::<RejectBlockHeaders>();
    m.visit::<RequestBlockHeaders>();
    m.visit::<RequestHeaderBlocks>();
    m.visit::<RejectHeaderBlocks>();
    m.visit::<RespondHeaderBlocks>();
    m.visit::<HeaderBlock>();
    m.visit::<UnfinishedHeaderBlock>();
    m.visit::<CoinState>();
    m.visit::<RegisterForPhUpdates>();
    m.visit::<RespondToPhUpdates>();
    m.visit::<RegisterForCoinUpdates>();
    m.visit::<RespondToCoinUpdates>();
    m.visit::<CoinStateUpdate>();
    m.visit::<RequestChildren>();
    m.visit::<RespondChildren>();
    m.visit::<RequestSesInfo>();
    m.visit::<RespondSesInfo>();
    m.visit::<RequestFeeEstimates>();
    m.visit::<RespondFeeEstimates>();
    m.visit::<RequestRemovePuzzleSubscriptions>();
    m.visit::<RespondRemovePuzzleSubscriptions>();
    m.visit::<RequestRemoveCoinSubscriptions>();
    m.visit::<RespondRemoveCoinSubscriptions>();
    m.visit::<CoinStateFilters>();
    m.visit::<RequestPuzzleState>();
    m.visit::<RespondPuzzleState>();
    m.visit::<RejectPuzzleState>();
    m.visit::<RequestCoinState>();
    m.visit::<RespondCoinState>();
    m.visit::<RejectCoinState>();

    // chia-protocol (full node)
    m.visit::<NewPeak>();
    m.visit::<NewTransaction>();
    m.visit::<RequestTransaction>();
    m.visit::<RespondTransaction>();
    m.visit::<RequestProofOfWeight>();
    m.visit::<RespondProofOfWeight>();
    m.visit::<RequestBlock>();
    m.visit::<RejectBlock>();
    m.visit::<RequestBlocks>();
    m.visit::<RespondBlocks>();
    m.visit::<RejectBlocks>();
    m.visit::<RespondBlock>();
    m.visit::<NewUnfinishedBlock>();
    m.visit::<RequestUnfinishedBlock>();
    m.visit::<RespondUnfinishedBlock>();
    m.visit::<NewSignagePointOrEndOfSubSlot>();
    m.visit::<RequestSignagePointOrEndOfSubSlot>();
    m.visit::<RespondSignagePoint>();
    m.visit::<RespondEndOfSubSlot>();
    m.visit::<RequestMempoolTransactions>();
    m.visit::<NewCompactVDF>();
    m.visit::<RequestCompactVDF>();
    m.visit::<RespondCompactVDF>();
    m.visit::<RequestPeers>();
    m.visit::<RespondPeers>();
    m.visit::<NewUnfinishedBlock2>();
    m.visit::<RequestUnfinishedBlock2>();

    // chia-bls
    m.visit::<AugSchemeMPL>();
    m.visit::<BlsCache>();
    m.visit::<PublicKey>();
    m.visit::<Signature>();
    m.visit::<GTElement>();
    m.visit::<SecretKey>();
}
