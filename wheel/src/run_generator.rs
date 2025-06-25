use chia_bls::{BlsCache, Signature};
use chia_consensus::additions_and_removals::additions_and_removals as native_additions_and_removals;
use chia_consensus::allocator::make_allocator;
use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::owned_conditions::OwnedSpendBundleConditions;
use chia_consensus::run_block_generator::run_block_generator as native_run_block_generator;
use chia_consensus::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia_consensus::validation_error::ValidationErr;
use chia_protocol::{Bytes, Bytes32, Coin};

use clvmr::cost::Cost;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::PyResult;

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
    block_refs: &Bound<'_, PyList>,
    max_cost: Cost,
    flags: u32,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> (Option<u32>, Option<OwnedSpendBundleConditions>) {
    let mut allocator = make_allocator(flags);

    let refs = block_refs
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs should be a list of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();
    let program = py_to_slice::<'a>(program);

    py.allow_threads(|| {
        match native_run_block_generator(
            &mut allocator,
            program,
            refs,
            max_cost,
            flags,
            signature,
            bls_cache,
            constants,
        ) {
            Ok(spend_bundle_conds) => (
                None,
                Some(OwnedSpendBundleConditions::from(
                    &allocator,
                    spend_bundle_conds,
                )),
            ),
            Err(ValidationErr(_, error_code)) => {
                // a validation error occurred
                (Some(error_code.into()), None)
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
    block_refs: &Bound<'_, PyList>,
    max_cost: Cost,
    flags: u32,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> (Option<u32>, Option<OwnedSpendBundleConditions>) {
    let mut allocator = make_allocator(flags);

    let refs = block_refs
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs must be list of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();

    let program = py_to_slice::<'a>(program);

    py.allow_threads(|| {
        match native_run_block_generator2(
            &mut allocator,
            program,
            refs,
            max_cost,
            flags,
            signature,
            bls_cache,
            constants,
        ) {
            Ok(spend_bundle_conds) => (
                None,
                Some(OwnedSpendBundleConditions::from(
                    &allocator,
                    spend_bundle_conds,
                )),
            ),
            Err(ValidationErr(_, error_code)) => {
                // a validation error occurred
                (Some(error_code.into()), None)
            }
        }
    })
}

#[pyfunction]
#[allow(clippy::type_complexity)]
pub fn additions_and_removals<'a>(
    py: Python<'a>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PyList>,
    flags: u32,
    constants: &ConsensusConstants,
) -> PyResult<(Vec<(Coin, Option<Bytes>)>, Vec<(Bytes32, Coin)>)> {
    let refs = block_refs
        .into_iter()
        .map(|b| {
            let buf = b
                .extract::<PyBuffer<u8>>()
                .expect("block_refs must be list of buffers");
            py_to_slice::<'a>(buf)
        })
        .collect::<Vec<&'a [u8]>>();

    let program = py_to_slice::<'a>(program);

    py.allow_threads(|| {
        native_additions_and_removals(program, refs, flags, constants).map_err(|e| {
            // a validation error occurred
            pyo3::exceptions::PyValueError::new_err(format!(
                "additions_and_removals() failed: {}",
                e.1 as u16
            ))
        })
    })
}
