use crate::error::{map_pyerr, map_pyerr_w_ptr};
use chia_consensus::allocator::make_allocator;
use chia_protocol::LazyNode;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Response;
use clvmr::run_program::run_program;
use clvmr::serde::{
    node_from_bytes_backrefs, serialized_length_from_bytes, serialized_length_from_bytes_trusted,
};
use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use std::rc::Rc;

#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn serialized_length(program: PyBuffer<u8>) -> PyResult<u64> {
    assert!(program.is_c_contiguous(), "program must be contiguous");
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };
    serialized_length_from_bytes(program).map_err(map_pyerr)
}

#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn serialized_length_trusted(program: PyBuffer<u8>) -> PyResult<u64> {
    assert!(program.is_c_contiguous(), "program must be contiguous");
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };
    serialized_length_from_bytes_trusted(program).map_err(map_pyerr)
}

#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn run_chia_program(
    py: Python<'_>,
    program: &[u8],
    args: &[u8],
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Cost, LazyNode)> {
    let mut allocator = make_allocator(flags);

    let reduction = (|| -> PyResult<Response> {
        let program = node_from_bytes_backrefs(&mut allocator, program)
            .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;
        let args = node_from_bytes_backrefs(&mut allocator, args)
            .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;
        let dialect = ChiaDialect::new(flags);

        Ok(py.allow_threads(|| run_program(&mut allocator, &dialect, program, args, max_cost)))
    })()?
    .map_err(|e| map_pyerr_w_ptr(&e, &allocator))?;
    let val = LazyNode::new(Rc::new(allocator), reduction.1);
    Ok((reduction.0, val))
}
