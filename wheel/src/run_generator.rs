use chia::allocator::make_allocator;
use chia::gen::conditions::SpendBundleConditions;
use chia::gen::run_block_generator::run_block_generator as native_run_block_generator;
use chia::gen::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia::gen::validation_error::ValidationErr;

use clvmr::cost::Cost;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::PyList;

#[pyfunction]
pub fn run_block_generator(
    _py: Python,
    program: PyBuffer<u8>,
    block_refs: &PyList,
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<SpendBundleConditions>)> {
    let mut allocator = make_allocator(flags);

    let mut refs = Vec::<&[u8]>::new();
    for g in block_refs {
        let buf = g.extract::<PyBuffer<u8>>()?;

        if !buf.is_c_contiguous() {
            panic!("block_refs buffers must be contiguous");
        }
        let slice =
            unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) };
        refs.push(slice);
    }

    if !program.is_c_contiguous() {
        panic!("program buffer must be contiguous");
    }
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    match native_run_block_generator(&mut allocator, program, &refs, max_cost, flags) {
        Ok(spend_bundle_conds) => {
            // everything was successful
            Ok((None, Some(spend_bundle_conds)))
        }
        Err(ValidationErr(_, error_code)) => {
            // a validation error occurred
            Ok((Some(error_code.into()), None))
        }
    }
}

#[pyfunction]
pub fn run_block_generator2(
    _py: Python,
    program: PyBuffer<u8>,
    block_refs: &PyList,
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<SpendBundleConditions>)> {
    let mut allocator = make_allocator(flags);

    let mut refs = Vec::<&[u8]>::new();
    for g in block_refs {
        let buf = g.extract::<PyBuffer<u8>>()?;

        if !buf.is_c_contiguous() {
            panic!("block_refs buffers must be contiguous");
        }
        let slice =
            unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) };
        refs.push(slice);
    }

    if !program.is_c_contiguous() {
        panic!("program buffer must be contiguous");
    }
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    match native_run_block_generator2(&mut allocator, program, &refs, max_cost, flags) {
        Ok(spend_bundle_conds) => {
            // everything was successful
            Ok((None, Some(spend_bundle_conds)))
        }
        Err(ValidationErr(_, error_code)) => {
            // a validation error occurred
            Ok((Some(error_code.into()), None))
        }
    }
}
