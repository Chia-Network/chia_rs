use crate::error::{map_pyerr, map_pyerr_w_ptr};
use crate::run_generator::{
    additions_and_removals, py_to_slice, run_block_generator, run_block_generator2,
};
use chia_consensus::allocator::make_allocator;
use chia_consensus::build_compressed_block::BlockBuilder;
use chia_consensus::check_time_locks::py_check_time_locks;
use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::flags::{
    COMPUTE_FINGERPRINT, COST_CONDITIONS, DONT_VALIDATE_SIGNATURE, MEMPOOL_MODE, NO_UNKNOWN_CONDS,
    STRICT_ARGS_COUNT,
};
use chia_consensus::merkle_set::compute_merkle_set_root as compute_merkle_root_impl;
use chia_consensus::merkle_tree::{validate_merkle_proof, MerkleSet};
use chia_consensus::owned_conditions::{OwnedSpendBundleConditions, OwnedSpendConditions};
use chia_consensus::run_block_generator::setup_generator_args;
use chia_consensus::run_block_generator::{
    get_coinspends_for_trusted_block, get_coinspends_with_conditions_for_trusted_block,
};
use chia_consensus::solution_generator::solution_generator as native_solution_generator;
use chia_consensus::solution_generator::solution_generator_backrefs as native_solution_generator_backrefs;
use chia_consensus::spendbundle_conditions::get_conditions_from_spendbundle;
use chia_consensus::spendbundle_validation::{
    get_flags_for_height_and_constants, validate_clvm_and_signature,
};
use chia_protocol::{
    calculate_ip_iters, calculate_sp_interval_iters, calculate_sp_iters, is_overflow_block,
    py_expected_plot_size,
};
use chia_protocol::{
    BlockRecord, Bytes32, ChallengeBlockInfo, ChallengeChainSubSlot, ClassgroupElement, Coin,
    CoinRecord, CoinSpend, CoinState, CoinStateFilters, CoinStateUpdate, EndOfSubSlotBundle,
    FeeEstimate, FeeEstimateGroup, FeeRate, Foliage, FoliageBlockData, FoliageTransactionBlock,
    FullBlock, Handshake, HeaderBlock, InfusedChallengeChainSubSlot, LazyNode, MempoolItemsAdded,
    MempoolItemsRemoved, Message, NewCompactVDF, NewPeak, NewPeakWallet,
    NewSignagePointOrEndOfSubSlot, NewTransaction, NewUnfinishedBlock, NewUnfinishedBlock2,
    PoolTarget, Program, ProofBlockHeader, ProofOfSpace, PuzzleSolutionResponse, PyPlotSize,
    RecentChainData, RegisterForCoinUpdates, RegisterForPhUpdates, RejectAdditionsRequest,
    RejectBlock, RejectBlockHeaders, RejectBlocks, RejectCoinState, RejectHeaderBlocks,
    RejectHeaderRequest, RejectPuzzleSolution, RejectPuzzleState, RejectRemovalsRequest,
    RemovedMempoolItem, RequestAdditions, RequestBlock, RequestBlockHeader, RequestBlockHeaders,
    RequestBlocks, RequestChildren, RequestCoinState, RequestCompactVDF, RequestCostInfo,
    RequestFeeEstimates, RequestHeaderBlocks, RequestMempoolTransactions, RequestPeers,
    RequestProofOfWeight, RequestPuzzleSolution, RequestPuzzleState, RequestRemovals,
    RequestRemoveCoinSubscriptions, RequestRemovePuzzleSubscriptions, RequestSesInfo,
    RequestSignagePointOrEndOfSubSlot, RequestTransaction, RequestUnfinishedBlock,
    RequestUnfinishedBlock2, RespondAdditions, RespondBlock, RespondBlockHeader,
    RespondBlockHeaders, RespondBlocks, RespondChildren, RespondCoinState, RespondCompactVDF,
    RespondCostInfo, RespondEndOfSubSlot, RespondFeeEstimates, RespondHeaderBlocks, RespondPeers,
    RespondProofOfWeight, RespondPuzzleSolution, RespondPuzzleState, RespondRemovals,
    RespondRemoveCoinSubscriptions, RespondRemovePuzzleSubscriptions, RespondSesInfo,
    RespondSignagePoint, RespondToCoinUpdates, RespondToPhUpdates, RespondTransaction,
    RespondUnfinishedBlock, RewardChainBlock, RewardChainBlockUnfinished, RewardChainSubSlot,
    SendTransaction, SpendBundle, SubEpochChallengeSegment, SubEpochData, SubEpochSegments,
    SubEpochSummary, SubSlotData, SubSlotProofs, TimestampedPeerInfo, TransactionAck,
    TransactionsInfo, UnfinishedBlock, UnfinishedHeaderBlock, VDFInfo, VDFProof, WeightProof,
};
use chia_traits::ChiaToPython;
use clvm_utils::tree_hash_from_bytes;
use clvmr::chia_dialect::ENABLE_KECCAK_OPS_OUTSIDE_GUARD;
use clvmr::{LIMIT_HEAP, NO_UNKNOWN_OPS};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::PyList;
use pyo3::types::PyTuple;
use pyo3::types::{PyBytes, PyCFunction, PyDict};
use pyo3::wrap_pyfunction;
use pyo3::PyClass;

use std::iter::zip;

use crate::run_program::{run_chia_program, serialized_length, serialized_length_trusted};

use chia_consensus::fast_forward::fast_forward_singleton as native_ff;
use chia_consensus::get_puzzle_and_solution::get_puzzle_and_solution_for_coin as parse_puzzle_solution;
use chia_consensus::validation_error::ValidationErr;
use clvmr::allocator::NodePtr;
use clvmr::cost::Cost;
use clvmr::error::EvalErr;
use clvmr::reduction::Reduction;
use clvmr::run_program;
use clvmr::serde::is_canonical_serialization;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs, node_to_bytes};
use clvmr::ChiaDialect;

use chia_bls::{
    hash_to_g2 as native_hash_to_g2, BlsCache, DerivableKey, GTElement, PublicKey, SecretKey,
    Signature,
};
#[pyfunction]
pub fn compute_merkle_set_root<'p>(
    py: Python<'p>,
    values: Vec<Bound<'p, PyBytes>>,
) -> PyResult<Bound<'p, PyBytes>> {
    let mut buffer = Vec::<[u8; 32]>::with_capacity(values.len());
    for b in values {
        use pyo3::types::PyBytesMethods;
        buffer.push(b.as_bytes().try_into()?);
    }
    Ok(PyBytes::new(py, &compute_merkle_root_impl(&mut buffer)))
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
pub fn tree_hash<'a>(py: Python<'a>, blob: PyBuffer<u8>) -> PyResult<Bound<'a, PyAny>> {
    let slice = py_to_slice::<'a>(blob);
    ChiaToPython::to_python(
        &Bytes32::from(&tree_hash_from_bytes(slice).map_err(map_pyerr)?.into()),
        py,
    )
}

// there is an updated version of this function that doesn't require serializing
// and deserializing the generator and arguments.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub fn get_puzzle_and_solution_for_coin<'a>(
    py: Python<'a>,
    program: PyBuffer<u8>,
    args: PyBuffer<u8>,
    max_cost: Cost,
    find_parent: Bytes32,
    find_amount: u64,
    find_ph: Bytes32,
    flags: u32,
) -> PyResult<(Bound<'a, PyBytes>, Bound<'a, PyBytes>)> {
    let mut allocator = make_allocator(LIMIT_HEAP);

    let program = py_to_slice::<'a>(program);
    let args = py_to_slice::<'a>(args);

    let program = node_from_bytes_backrefs(&mut allocator, program)
        .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;
    let args = node_from_bytes_backrefs(&mut allocator, args)
        .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;
    let dialect = &ChiaDialect::new(flags);

    let (puzzle, solution) = py
        .allow_threads(|| -> Result<(NodePtr, NodePtr), EvalErr> {
            let Reduction(_cost, result) =
                run_program(&mut allocator, dialect, program, args, max_cost)?;
            match parse_puzzle_solution(
                &allocator,
                result,
                &Coin::new(find_parent, find_ph, find_amount),
            ) {
                Err(ValidationErr(n, _)) => {
                    Err(EvalErr::InvalidOpArg(n, "coin not found".to_string()))
                }
                Ok(pair) => Ok(pair),
            }
        })
        .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;

    // keep serializing normally, until wallets support backrefs
    let serialize = node_to_bytes;
    Ok((
        PyBytes::new(
            py,
            &serialize(&allocator, puzzle).map_err(|e| map_pyerr_w_ptr(&e, &allocator))?,
        ),
        PyBytes::new(
            py,
            &serialize(&allocator, solution).map_err(|e| map_pyerr_w_ptr(&e, &allocator))?,
        ),
    ))
}

// This is a new version of get_puzzle_and_solution_for_coin() which uses the
// right types for generator, blocks_refs and the return value.
// The old version was written when Program was a python type had to be
// serialized to bytes through rust boundary.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub fn get_puzzle_and_solution_for_coin2<'a>(
    py: Python<'a>,
    generator: &Program,
    block_refs: &Bound<'a, PyList>,
    max_cost: Cost,
    find_coin: &Coin,
    flags: u32,
) -> PyResult<(Program, Program)> {
    let mut allocator = make_allocator(LIMIT_HEAP);

    let refs = block_refs.into_iter().map(|b| {
        let buf = b
            .extract::<PyBuffer<u8>>()
            .expect("block_refs should be a list of buffers");
        py_to_slice::<'a>(buf)
    });

    let generator = node_from_bytes_backrefs(&mut allocator, generator.as_ref())
        .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;
    let args = setup_generator_args(&mut allocator, refs)?;
    let dialect = &ChiaDialect::new(flags);

    let (puzzle, solution) = py
        .allow_threads(|| -> Result<(NodePtr, NodePtr), EvalErr> {
            let Reduction(_cost, result) =
                run_program(&mut allocator, dialect, generator, args, max_cost)?;
            match parse_puzzle_solution(&allocator, result, find_coin) {
                Err(ValidationErr(n, _)) => {
                    Err(EvalErr::InvalidOpArg(n, "coin not found".to_string()))
                }
                Ok(pair) => Ok(pair),
            }
        })
        .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;

    // keep serializing normally, until wallets support backrefs
    Ok((
        node_to_bytes(&allocator, puzzle)
            .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?
            .into(),
        node_to_bytes(&allocator, solution)
            .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?
            .into(),
    ))
}

// this is like a CoinSpend but with references to the puzzle and solution,
// rather than owning them
type CoinSpendRef = (Coin, PyBackedBytes, PyBackedBytes);

fn convert_list_of_tuples(spends: &Bound<'_, PyAny>) -> PyResult<Vec<CoinSpendRef>> {
    let mut native_spends = Vec::<CoinSpendRef>::new();
    for s in spends.try_iter()? {
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
    Ok(PyBytes::new(py, &native_solution_generator(spends)?))
}

#[pyfunction]
fn solution_generator_backrefs<'p>(
    py: Python<'p>,
    spends: &Bound<'_, PyAny>,
) -> PyResult<Bound<'p, PyBytes>> {
    let spends = convert_list_of_tuples(spends)?;
    Ok(PyBytes::new(
        py,
        &native_solution_generator_backrefs(spends)?,
    ))
}

#[pyclass]
struct AugSchemeMPL {}

#[pymethods]
impl AugSchemeMPL {
    #[staticmethod]
    #[pyo3(signature = (pk,msg,prepend_pk=None))]
    pub fn sign(pk: &SecretKey, msg: &[u8], prepend_pk: Option<&PublicKey>) -> Signature {
        match prepend_pk {
            Some(prefix) => {
                let mut aug_msg = prefix.to_bytes().to_vec();
                aug_msg.extend_from_slice(msg);
                chia_bls::sign_raw(pk, aug_msg)
            }
            None => chia_bls::sign(pk, msg),
        }
    }

    #[staticmethod]
    pub fn aggregate(sigs: &Bound<'_, PyList>) -> PyResult<Signature> {
        let mut ret = Signature::default();
        for p2 in sigs {
            ret += &p2.extract::<Signature>()?;
        }
        Ok(ret)
    }

    #[staticmethod]
    pub fn verify(py: Python<'_>, pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool {
        py.allow_threads(|| chia_bls::verify(sig, pk, msg))
    }

    #[staticmethod]
    pub fn aggregate_verify(
        py: Python<'_>,
        pks: &Bound<'_, PyList>,
        msgs: &Bound<'_, PyList>,
        sig: &Signature,
    ) -> PyResult<bool> {
        let mut data = Vec::<(PublicKey, Vec<u8>)>::new();
        if pks.len() != msgs.len() {
            return Err(PyRuntimeError::new_err(
                "aggregate_verify expects the same number of public keys as messages",
            ));
        }
        for (pk, msg) in zip(pks, msgs) {
            let pk = pk.extract::<PublicKey>()?;
            let msg = msg.extract::<Vec<u8>>()?;
            data.push((pk, msg));
        }

        py.allow_threads(|| Ok(chia_bls::aggregate_verify(sig, data)))
    }

    #[staticmethod]
    pub fn g2_from_message(msg: &[u8]) -> Signature {
        native_hash_to_g2(msg)
    }

    #[staticmethod]
    pub fn derive_child_sk(sk: &SecretKey, index: u32) -> SecretKey {
        sk.derive_hardened(index)
    }

    #[staticmethod]
    pub fn derive_child_sk_unhardened(sk: &SecretKey, index: u32) -> SecretKey {
        sk.derive_unhardened(index)
    }

    #[staticmethod]
    pub fn derive_child_pk_unhardened(pk: &PublicKey, index: u32) -> PublicKey {
        pk.derive_unhardened(index)
    }

    #[staticmethod]
    pub fn key_gen(seed: &[u8]) -> PyResult<SecretKey> {
        if seed.len() < 32 {
            return Err(PyRuntimeError::new_err(
                "Seed size must be at leat 32 bytes",
            ));
        }
        Ok(SecretKey::from_seed(seed))
    }
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
    let puzzle = node_from_bytes(&mut a, spend.puzzle_reveal.as_slice())
        .map_err(|e| map_pyerr_w_ptr(&e, &a))?;
    let solution =
        node_from_bytes(&mut a, spend.solution.as_slice()).map_err(|e| map_pyerr_w_ptr(&e, &a))?;

    let new_solution = native_ff(&mut a, puzzle, solution, &spend.coin, new_coin, new_parent)?;
    Ok(PyBytes::new(
        py,
        node_to_bytes(&a, new_solution)
            .map_err(|e| map_pyerr_w_ptr(&e, &a))?
            .as_slice(),
    ))
}

#[pyfunction]
#[pyo3(name = "validate_clvm_and_signature")]
#[allow(clippy::type_complexity)]
pub fn py_validate_clvm_and_signature(
    py: Python<'_>,
    new_spend: &SpendBundle,
    max_cost: u64,
    constants: &ConsensusConstants,
    flags: u32,
) -> PyResult<(OwnedSpendBundleConditions, Vec<([u8; 32], GTElement)>, f32)> {
    let (owned_conditions, additions, duration) =
        py.allow_threads(|| validate_clvm_and_signature(new_spend, max_cost, constants, flags))?;
    Ok((owned_conditions, additions, duration.as_secs_f32()))
}

#[pyfunction]
#[pyo3(name = "get_conditions_from_spendbundle")]
pub fn py_get_conditions_from_spendbundle(
    spend_bundle: &SpendBundle,
    max_cost: u64,
    constants: &ConsensusConstants,
    height: u32,
) -> PyResult<OwnedSpendBundleConditions> {
    use chia_consensus::allocator::make_allocator;
    use chia_consensus::owned_conditions::OwnedSpendBundleConditions;
    let mut a = make_allocator(LIMIT_HEAP);
    let conditions =
        get_conditions_from_spendbundle(&mut a, spend_bundle, max_cost, height, constants)?;
    Ok(OwnedSpendBundleConditions::from(&a, conditions))
}

#[pyfunction]
#[pyo3(name = "get_flags_for_height_and_constants")]
pub fn py_get_flags_for_height_and_constants(height: u32, constants: &ConsensusConstants) -> u32 {
    get_flags_for_height_and_constants(height, constants)
}

#[pyo3::pyfunction]
#[pyo3(name = "is_overflow_block")]
pub fn py_is_overflow_block(
    constants: &ConsensusConstants,
    signage_point_index: u8,
) -> pyo3::PyResult<bool> {
    Ok(is_overflow_block(
        constants.num_sps_sub_slot,
        constants.num_sp_intervals_extra,
        signage_point_index,
    )?)
}

#[pyo3::pyfunction]
#[pyo3(name = "calculate_sp_interval_iters")]
pub fn py_calculate_sp_interval_iters(
    constants: &ConsensusConstants,
    sub_slot_iters: u64,
) -> pyo3::PyResult<u64> {
    Ok(calculate_sp_interval_iters(
        constants.num_sps_sub_slot,
        sub_slot_iters,
    )?)
}

#[pyo3::pyfunction]
#[pyo3(name = "calculate_sp_iters")]
pub fn py_calculate_sp_iters(
    constants: &ConsensusConstants,
    sub_slot_iters: u64,
    signage_point_index: u8,
) -> pyo3::PyResult<u64> {
    Ok(calculate_sp_iters(
        constants.num_sps_sub_slot,
        sub_slot_iters,
        signage_point_index,
    )?)
}

#[pyo3::pyfunction]
#[pyo3(name = "calculate_ip_iters")]
pub fn py_calculate_ip_iters(
    constants: &ConsensusConstants,
    sub_slot_iters: u64,
    signage_point_index: u8,
    required_iters: u64,
) -> pyo3::PyResult<u64> {
    Ok(calculate_ip_iters(
        constants.num_sps_sub_slot,
        constants.num_sp_intervals_extra,
        sub_slot_iters,
        signage_point_index,
        required_iters,
    )?)
}

#[pyo3::pyfunction]
pub fn get_spends_for_trusted_block<'a>(
    py: Python<'a>,
    constants: &ConsensusConstants,
    generator: Program,
    block_refs: &Bound<'_, PyList>,
    flags: u32,
) -> pyo3::PyResult<PyObject> {
    let refs = block_refs
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs must be list of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();

    let output =
        py.allow_threads(|| get_coinspends_for_trusted_block(constants, &generator, &refs, flags))?;

    let dict = PyDict::new(py);
    dict.set_item("block_spends", output)?;
    Ok(dict.into())
}

#[pyo3::pyfunction]
pub fn get_spends_for_trusted_block_with_conditions<'a>(
    py: Python<'a>,
    constants: &ConsensusConstants,
    generator: Program,
    block_refs: &Bound<'a, PyList>,
    flags: u32,
) -> pyo3::PyResult<PyObject> {
    let refs = block_refs
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs must be list of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();

    let output = py.allow_threads(|| {
        get_coinspends_with_conditions_for_trusted_block(constants, &generator, &refs, flags)
    })?;

    let pylist = PyList::empty(py);
    for (coinspend, cond_output) in output {
        let dict = PyDict::new(py);
        dict.set_item("coin_spend", coinspend)?;
        let cond_list = PyList::empty(py);
        for (opcode, bytes_vec) in cond_output {
            let arg_list = PyList::empty(py);
            for bytes in bytes_vec {
                let pybytes = PyBytes::new(py, bytes.as_slice());
                arg_list.append(pybytes)?;
            }

            let tuple = (opcode, arg_list);
            cond_list.append(tuple)?;
        }

        dict.set_item("conditions", cond_list)?;
        pylist.append(dict)?;
    }
    Ok(pylist.into())
}

#[pyo3::pyfunction]
#[pyo3(name = "is_canonical_serialization")]
pub fn py_is_canonical_serialization(buf: &[u8]) -> bool {
    is_canonical_serialization(buf)
}

fn get_fixed_module_path(m: &Bound<'_, PyModule>) -> PyResult<String> {
    let fixed = fix_import_path(m.name()?.extract::<String>()?.as_str());

    Ok(fixed)
}

fn fix_import_path(path_string: &str) -> String {
    path_string.replace(".chia_rs", "")
}
fn add_class<T>(m: &Bound<'_, PyModule>) -> PyResult<()>
where
    T: PyClass,
{
    m.add_class::<T>()?;
    let cls = m.getattr(T::NAME)?;
    cls.setattr("__module__", get_fixed_module_path(m)?)?;

    Ok(())
}

fn add_function(m: &Bound<'_, PyModule>, wrapped: Bound<'_, PyCFunction>) -> PyResult<()> {
    wrapped.setattr("__module__", get_fixed_module_path(m)?)?;

    m.add_function(wrapped)?;
    Ok(())
}

macro_rules! add_functions {
    ($m:expr, $($func:ident),+ $(,)?) => {
      $(
          add_function($m, wrap_pyfunction!($func, $m)?)?;
      )+
    };
}

macro_rules! add_classes {
    ($m:expr, $($class:ident),+ $(,)?) => {
      $(
          add_class::<$class>($m)?;
      )+
    };
}

#[pymodule]
pub fn chia_rs(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // generator functions
    add_functions!(
        m,
        run_block_generator,
        run_block_generator2,
        additions_and_removals,
        solution_generator,
        solution_generator_backrefs,
        supports_fast_forward,
        fast_forward_singleton,
    );
    add_classes!(m, OwnedSpendBundleConditions, BlockBuilder,);
    m.add(
        "ELIGIBLE_FOR_DEDUP",
        chia_consensus::conditions::ELIGIBLE_FOR_DEDUP,
    )?;
    m.add(
        "ELIGIBLE_FOR_FF",
        chia_consensus::conditions::ELIGIBLE_FOR_FF,
    )?;
    add_classes!(m, OwnedSpendConditions,);

    add_functions!(
        m,
        // pot functions
        py_calculate_sp_interval_iters,
        py_calculate_sp_iters,
        py_calculate_ip_iters,
        py_is_overflow_block,
        py_expected_plot_size,
        // check time lock
        py_check_time_locks,
        // CLVM validation
        py_is_canonical_serialization,
    );

    // constants
    add_classes!(m, ConsensusConstants,);

    // merkle tree
    add_classes!(m, MerkleSet,);
    add_functions!(
        m,
        confirm_included_already_hashed,
        confirm_not_included_already_hashed,
        // spendbundle validation
        py_validate_clvm_and_signature,
        py_get_conditions_from_spendbundle,
        py_get_flags_for_height_and_constants,
        // get spends for generator
        get_spends_for_trusted_block,
        get_spends_for_trusted_block_with_conditions,
    );

    // clvm functions
    m.add("NO_UNKNOWN_CONDS", NO_UNKNOWN_CONDS)?;
    m.add("STRICT_ARGS_COUNT", STRICT_ARGS_COUNT)?;
    m.add("MEMPOOL_MODE", MEMPOOL_MODE)?;
    m.add("DONT_VALIDATE_SIGNATURE", DONT_VALIDATE_SIGNATURE)?;
    m.add("COMPUTE_FINGERPRINT", COMPUTE_FINGERPRINT)?;
    m.add("COST_CONDITIONS", COST_CONDITIONS)?;

    // for backwards compatibility
    m.add("ALLOW_BACKREFS", 0)?;

    add_classes!(
        m,
        PyPlotSize,
        // Chia classes
        Coin,
        CoinRecord,
        PoolTarget,
        ClassgroupElement,
        EndOfSubSlotBundle,
        TransactionsInfo,
        FoliageTransactionBlock,
        FoliageBlockData,
        Foliage,
        ProofOfSpace,
        RewardChainBlockUnfinished,
        RewardChainBlock,
        ChallengeBlockInfo,
        ChallengeChainSubSlot,
        InfusedChallengeChainSubSlot,
        RewardChainSubSlot,
        SubSlotProofs,
        SpendBundle,
        Program,
        CoinSpend,
        VDFInfo,
        VDFProof,
        SubSlotData,
        SubEpochData,
        SubEpochChallengeSegment,
        SubEpochSegments,
        SubEpochSummary,
        UnfinishedBlock,
        FullBlock,
        BlockRecord,
        WeightProof,
        RecentChainData,
        ProofBlockHeader,
        TimestampedPeerInfo,
        // wallet protocol
        RequestPuzzleSolution,
        PuzzleSolutionResponse,
        RespondPuzzleSolution,
        RejectPuzzleSolution,
        SendTransaction,
        TransactionAck,
        NewPeakWallet,
        RequestBlockHeader,
        RespondBlockHeader,
        RejectHeaderRequest,
        RequestRemovals,
        RespondRemovals,
        RejectRemovalsRequest,
        RequestAdditions,
        RespondAdditions,
        RejectAdditionsRequest,
        RespondBlockHeaders,
        RejectBlockHeaders,
        RequestBlockHeaders,
        RequestHeaderBlocks,
        RejectHeaderBlocks,
        RespondHeaderBlocks,
        HeaderBlock,
        UnfinishedHeaderBlock,
        CoinState,
        RegisterForPhUpdates,
        RespondToPhUpdates,
        RegisterForCoinUpdates,
        RespondToCoinUpdates,
        CoinStateUpdate,
        RequestChildren,
        RespondChildren,
        RequestSesInfo,
        RespondSesInfo,
        RequestFeeEstimates,
        RespondFeeEstimates,
        RequestRemovePuzzleSubscriptions,
        RespondRemovePuzzleSubscriptions,
        RequestRemoveCoinSubscriptions,
        RespondRemoveCoinSubscriptions,
        CoinStateFilters,
        RequestPuzzleState,
        RespondPuzzleState,
        RejectPuzzleState,
        RequestCoinState,
        RespondCoinState,
        RejectCoinState,
        MempoolItemsAdded,
        MempoolItemsRemoved,
        RemovedMempoolItem,
        RequestCostInfo,
        RespondCostInfo,
        // full node protocol
        NewPeak,
        NewTransaction,
        RequestTransaction,
        RespondTransaction,
        RequestProofOfWeight,
        RespondProofOfWeight,
        RequestBlock,
        RejectBlock,
        RequestBlocks,
        RespondBlocks,
        RejectBlocks,
        RespondBlock,
        NewUnfinishedBlock,
        RequestUnfinishedBlock,
        RespondUnfinishedBlock,
        NewSignagePointOrEndOfSubSlot,
        RequestSignagePointOrEndOfSubSlot,
        RespondSignagePoint,
        RespondEndOfSubSlot,
        RequestMempoolTransactions,
        NewCompactVDF,
        RequestCompactVDF,
        RespondCompactVDF,
        RequestPeers,
        RespondPeers,
        NewUnfinishedBlock2,
        RequestUnfinishedBlock2,
        Handshake,
        FeeEstimate,
        FeeEstimateGroup,
        FeeRate,
        LazyNode,
        Message,
    );

    // facilities from clvm_rs

    add_functions!(m, run_chia_program,);
    m.add("NO_UNKNOWN_OPS", NO_UNKNOWN_OPS)?;
    m.add("LIMIT_HEAP", LIMIT_HEAP)?;
    m.add(
        "ENABLE_KECCAK_OPS_OUTSIDE_GUARD",
        ENABLE_KECCAK_OPS_OUTSIDE_GUARD,
    )?;

    add_functions!(
        m,
        serialized_length,
        serialized_length_trusted,
        compute_merkle_set_root,
        tree_hash,
        get_puzzle_and_solution_for_coin,
        get_puzzle_and_solution_for_coin2,
    );

    // facilities from chia-bls

    add_classes!(
        m,
        PublicKey,
        Signature,
        GTElement,
        SecretKey,
        AugSchemeMPL,
        BlsCache,
    );

    add_datalayer_submodule(py, m)?;

    Ok(())
}

pub fn add_datalayer_submodule(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    use chia_datalayer::*;

    let single_element_module_name = "datalayer";
    let original_path = format!("{}.{}", parent.name()?, single_element_module_name);
    let module_path = fix_import_path(&original_path);
    let module = PyModule::new(py, &module_path)?;
    parent.add_submodule(&module)?;

    // https://github.com/PyO3/pyo3/pull/5375
    // move attribute to proper name
    parent.delattr(module.name()?)?;
    parent.setattr(single_element_module_name, &module)?;
    // update __all__ as well
    let all = parent.getattr("__all__")?;
    all.call_method1("remove", (module.name()?,))?;
    all.call_method1("append", (single_element_module_name,))?;

    add_classes!(
        &module,
        BlockStatusCache,
        DeltaReader,
        MerkleBlob,
        InternalNode,
        LeafNode,
        KeyId,
        ValueId,
        TreeIndex,
        ProofOfInclusionLayer,
        ProofOfInclusion,
        DeltaFileCache,
    );

    module.add("BLOCK_SIZE", BLOCK_SIZE)?;
    module.add("DATA_SIZE", DATA_SIZE)?;
    module.add("METADATA_SIZE", METADATA_SIZE)?;

    python_exceptions::add_to_module(py, &module)?;

    // https://github.com/PyO3/pyo3/issues/1517#issuecomment-808664021
    // https://github.com/PyO3/pyo3/issues/759
    // needed for: import chia_rs.datalayer
    // not needed for: from chia_rs import datalayer
    py.import("sys")?
        .getattr("modules")?
        .set_item("chia_rs.datalayer", module)?;

    Ok(())
}
