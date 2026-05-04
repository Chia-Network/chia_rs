use chia_bls::{BlsCache, Signature};
use chia_consensus::additions_and_removals::additions_and_removals as native_additions_and_removals;
use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::generator_cost::interned_vbytes;
use chia_consensus::owned_conditions::OwnedSpendBundleConditions;
use chia_consensus::run_block_generator::run_block_generator as native_run_block_generator;
use chia_consensus::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia_protocol::{Bytes, Bytes32, Coin};

use chia_consensus::program_bytes::node_from_bytes_auto;
use clvmr::allocator::Allocator;
use clvmr::cost::Cost;
use clvmr::serde::intern_tree_limited;

use pyo3::PyResult;
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PySequence, PySequenceMethods};

pub fn py_to_slice<'a>(buf: PyBuffer<u8>) -> &'a [u8] {
    assert!(buf.is_c_contiguous(), "buffer must be contiguous");
    unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) }
}

#[pyfunction]
#[pyo3(signature = (program, block_refs, max_cost, flags, signature, bls_cache, constants))]
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator<'a>(
    py: Python<'a>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PySequence>,
    max_cost: Cost,
    flags: ConsensusFlags,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> (
    Option<u32>,
    Option<String>,
    Option<OwnedSpendBundleConditions>,
) {
    let refs = block_refs
        .to_list()
        .expect("block_refs should be a sequence")
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs should be a sequence of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();
    let program = py_to_slice::<'a>(program);

    py.detach(|| {
        match native_run_block_generator(
            program, refs, max_cost, flags, signature, bls_cache, constants,
        ) {
            Ok((allocator, spend_bundle_conds)) => (
                None,
                None,
                Some(OwnedSpendBundleConditions::from(
                    &allocator,
                    spend_bundle_conds,
                )),
            ),
            Err(e) => {
                let code = e.error_code();
                (Some(code.into()), Some(format!("{e}")), None)
            }
        }
    })
}

#[pyfunction]
#[pyo3(signature = (program, block_refs, max_cost, flags, signature, bls_cache, constants))]
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator2<'a>(
    py: Python<'a>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PySequence>,
    max_cost: Cost,
    flags: ConsensusFlags,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> (
    Option<u32>,
    Option<String>,
    Option<OwnedSpendBundleConditions>,
) {
    let refs = block_refs
        .to_list()
        .expect("block_refs should be a sequence")
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs must be sequence of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();

    let program = py_to_slice::<'a>(program);

    py.detach(|| {
        match native_run_block_generator2(
            program, refs, max_cost, flags, signature, bls_cache, constants,
        ) {
            Ok((allocator, spend_bundle_conds)) => (
                None,
                None,
                Some(OwnedSpendBundleConditions::from(
                    &allocator,
                    spend_bundle_conds,
                )),
            ),
            Err(e) => {
                let code = e.error_code();
                (Some(code.into()), Some(format!("{e}")), None)
            }
        }
    })
}

#[pyfunction]
#[allow(clippy::type_complexity)]
pub fn additions_and_removals<'a>(
    py: Python<'a>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PySequence>,
    flags: ConsensusFlags,
    constants: &ConsensusConstants,
) -> PyResult<(Vec<(Coin, Option<Bytes>)>, Vec<(Bytes32, Coin)>)> {
    let refs = block_refs
        .to_list()?
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs must be sequence of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();

    let program = py_to_slice::<'a>(program);

    py.detach(|| {
        native_additions_and_removals(program, refs, flags, constants)
            .map_err(|e| -> pyo3::PyErr { e.into() })
    })
}

/// Return the byte-weight-equivalent of a serialized generator program.
///
/// Deserializes (with back-refs), interns the tree, and returns
/// `atom_bytes + 2*atom_count + 3*pair_count`.  Multiply by
/// `cost_per_byte` from consensus constants to get the full generator size cost.
#[pyfunction]
pub fn generator_interned_vbytes(py: Python<'_>, program: PyBuffer<u8>) -> PyResult<u64> {
    let program = py_to_slice(program);
    py.detach(|| {
        let mut a = Allocator::new();
        let node = node_from_bytes_backrefs(&mut a, program)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("bad generator: {e}")))?;
        let tree = intern_tree_limited(&a, node, u32::MAX as usize)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("intern failed: {e}")))?;
        Ok(interned_vbytes(&tree))
    })
}
