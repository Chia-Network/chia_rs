use chia_consensus::allocator::make_allocator;
use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::gen::conditions::{EmptyVisitor, MempoolVisitor};
use chia_consensus::gen::flags::ANALYZE_SPENDS;
use chia_consensus::gen::owned_conditions::OwnedSpendBundleConditions;
use chia_consensus::gen::run_block_generator::run_block_generator as native_run_block_generator;
use chia_consensus::gen::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia_consensus::gen::validation_error::ValidationErr;

use clvmr::cost::Cost;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::PyList;

pub fn py_to_slice<'a>(buf: PyBuffer<u8>) -> &'a [u8] {
    assert!(buf.is_c_contiguous(), "buffer must be contiguous");
    unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) }
}

#[pyfunction]
pub fn run_block_generator<'a>(
    _py: Python<'a>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PyList>,
    max_cost: Cost,
    flags: u32,
    constants: &ConsensusConstants,
) -> (Option<u32>, Option<OwnedSpendBundleConditions>) {
    let mut allocator = make_allocator(flags);

    let refs = block_refs.into_iter().map(|b| {
        let buf = b
            .extract::<PyBuffer<u8>>()
            .expect("block_refs should be a list of buffers");
        py_to_slice::<'a>(buf)
    });
    let program = py_to_slice::<'a>(program);
    let run_block = if (flags & ANALYZE_SPENDS) == 0 {
        native_run_block_generator::<_, EmptyVisitor, _>
    } else {
        native_run_block_generator::<_, MempoolVisitor, _>
    };

    match run_block(&mut allocator, program, refs, max_cost, flags, constants) {
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
}

#[pyfunction]
pub fn run_block_generator2<'a>(
    _py: Python<'a>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PyList>,
    max_cost: Cost,
    flags: u32,
    constants: &ConsensusConstants,
) -> (Option<u32>, Option<OwnedSpendBundleConditions>) {
    let mut allocator = make_allocator(flags);

    let refs = block_refs.into_iter().map(|b| {
        let buf = b
            .extract::<PyBuffer<u8>>()
            .expect("block_refs must be list of buffers");
        py_to_slice::<'a>(buf)
    });

    let program = py_to_slice::<'a>(program);
    let run_block = if (flags & ANALYZE_SPENDS) == 0 {
        native_run_block_generator2::<_, EmptyVisitor, _>
    } else {
        native_run_block_generator2::<_, MempoolVisitor, _>
    };

    match run_block(&mut allocator, program, refs, max_cost, flags, constants) {
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
}
