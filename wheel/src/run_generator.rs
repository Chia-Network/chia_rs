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

#[pyfunction]
pub fn run_block_generator(
    _py: Python<'_>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PyList>,
    max_cost: Cost,
    flags: u32,
    constants: &ConsensusConstants,
) -> PyResult<(Option<u32>, Option<OwnedSpendBundleConditions>)> {
    let mut allocator = make_allocator(flags);

    let mut refs = Vec::<&[u8]>::new();
    for g in block_refs {
        let buf = g.extract::<PyBuffer<u8>>()?;

        assert!(
            buf.is_c_contiguous(),
            "block_refs buffers must be contiguous"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) };
        refs.push(slice);
    }

    assert!(
        program.is_c_contiguous(),
        "program buffer must be contiguous"
    );
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    let run_block = if (flags & ANALYZE_SPENDS) == 0 {
        native_run_block_generator::<_, EmptyVisitor>
    } else {
        native_run_block_generator::<_, MempoolVisitor>
    };

    Ok(
        match run_block(&mut allocator, program, &refs, max_cost, flags, constants) {
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
        },
    )
}

#[pyfunction]
pub fn run_block_generator2(
    _py: Python<'_>,
    program: PyBuffer<u8>,
    block_refs: &Bound<'_, PyList>,
    max_cost: Cost,
    flags: u32,
    constants: &ConsensusConstants,
) -> PyResult<(Option<u32>, Option<OwnedSpendBundleConditions>)> {
    let mut allocator = make_allocator(flags);

    let mut refs = Vec::<&[u8]>::new();
    for g in block_refs {
        let buf = g.extract::<PyBuffer<u8>>()?;

        assert!(
            buf.is_c_contiguous(),
            "block_refs buffers must be contiguous"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) };
        refs.push(slice);
    }

    assert!(
        program.is_c_contiguous(),
        "program buffer must be contiguous"
    );
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    let run_block = if (flags & ANALYZE_SPENDS) == 0 {
        native_run_block_generator2::<_, EmptyVisitor>
    } else {
        native_run_block_generator2::<_, MempoolVisitor>
    };

    Ok(
        match run_block(&mut allocator, program, &refs, max_cost, flags, constants) {
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
        },
    )
}
