use chia_bls::{BlsCache, Signature};
use chia_consensus::additions_and_removals::additions_and_removals as native_additions_and_removals;
use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::generator_cost::interned_vbytes;
use chia_consensus::owned_conditions::OwnedSpendBundleConditions;
use chia_consensus::run_block_generator::run_block_generator as native_run_block_generator;
use chia_consensus::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia_protocol::{Bytes, Bytes32, Coin};

use chia_consensus::serde_2026::node_from_bytes_auto;
use clvmr::allocator::Allocator;
use clvmr::cost::Cost;
use clvmr::serde::intern_tree_limited;

use pyo3::PyResult;
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::{PySequence, PySequenceMethods};

pub fn py_to_slice(buf: &PyBuffer<u8>) -> &[u8] {
    assert!(buf.is_c_contiguous(), "buffer must be contiguous");
    unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) }
}

/// Keep these alive while using slices from `py_to_slice`; dropping releases the
/// export and can invalidate views (e.g. `memoryview`).
pub fn extract_buffers(block_refs: &Bound<'_, PySequence>) -> PyResult<Vec<PyBuffer<u8>>> {
    Ok(block_refs
        .to_list()?
        .into_iter()
        .map(|b| {
            b.extract::<PyBuffer<u8>>()
                .expect("block_refs must be a sequence of buffers")
        })
        .collect())
}

#[pyfunction]
#[pyo3(signature = (program, block_refs, max_cost, flags, signature, bls_cache, constants))]
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator(
    py: Python<'_>,
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
    let block_ref_buffers = extract_buffers(block_refs).expect("block_refs should be a sequence");
    let refs: Vec<&[u8]> = block_ref_buffers.iter().map(py_to_slice).collect();
    let program = py_to_slice(&program);

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
pub fn run_block_generator2(
    py: Python<'_>,
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
    let block_ref_buffers = extract_buffers(block_refs).expect("block_refs should be a sequence");
    let refs: Vec<&[u8]> = block_ref_buffers.iter().map(py_to_slice).collect();
    let program = py_to_slice(&program);

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
pub fn additions_and_removals(
    py: Python<'_>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PySequence>,
    flags: ConsensusFlags,
    constants: &ConsensusConstants,
) -> PyResult<(Vec<(Coin, Option<Bytes>)>, Vec<(Bytes32, Coin)>)> {
    let block_ref_buffers = extract_buffers(block_refs)?;
    let refs: Vec<&[u8]> = block_ref_buffers.iter().map(py_to_slice).collect();
    let program = py_to_slice(&program);

    py.detach(|| {
        native_additions_and_removals(program, refs, flags, constants)
            .map_err(|e| -> pyo3::PyErr { e.into() })
    })
}

/// Return the byte-weight-equivalent of a serialized generator program.
///
/// Deserializes (accepting classic, back-refs and serde_2026 encodings),
/// interns the tree, and returns
/// `atom_bytes + 2*atom_count + 3*pair_count`.  Multiply by
/// `cost_per_byte` from consensus constants to get the full generator size cost.
#[pyfunction]
pub fn generator_interned_vbytes(py: Python<'_>, program: PyBuffer<u8>) -> PyResult<u64> {
    let program = py_to_slice(&program);
    py.detach(|| {
        let mut a = Allocator::new();
        // trusted input; no size cap, like the back-refs path had none
        let node = node_from_bytes_auto(&mut a, program, usize::MAX)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("bad generator: {e}")))?;
        let tree = intern_tree_limited(&a, node, u32::MAX as usize)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("intern failed: {e}")))?;
        Ok(interned_vbytes(&tree))
    })
}
