use crate::error::map_pyerr;
use chia_consensus::allocator::make_allocator;
use chia_consensus::flags::ConsensusFlags;
use chia_protocol::LazyNode;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Response;
use clvmr::run_program::{run_program, run_program_with_timeout};
use clvmr::serde::{
    node_from_bytes_backrefs, serialized_length_from_bytes, serialized_length_from_bytes_trusted,
};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::rc::Rc;
use std::time::Duration;

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
    flags: ConsensusFlags,
) -> PyResult<(Cost, LazyNode)> {
    let mut allocator = make_allocator(flags);
    let flags = flags.to_clvm_flags();

    let reduction = (|| -> PyResult<Response> {
        let program = node_from_bytes_backrefs(&mut allocator, program).map_err(map_pyerr)?;
        let args = node_from_bytes_backrefs(&mut allocator, args).map_err(map_pyerr)?;
        let dialect = ChiaDialect::new(flags);

        Ok(py.detach(|| run_program(&mut allocator, &dialect, program, args, max_cost)))
    })()?
    .map_err(map_pyerr)?;
    let val = LazyNode::new(Rc::new(allocator), reduction.1);
    Ok((reduction.0, val))
}

/// Run a CLVM program with a wall-clock timeout.
///
/// `timeout` is in seconds. The timeout is checked roughly every 1_000_000 cost
/// units; programs that finish before the first check are unaffected. If the
/// elapsed time exceeds `timeout`, raises `ValueError("timeout")`.
#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn run_chia_program_with_timeout(
    py: Python<'_>,
    program: &[u8],
    args: &[u8],
    max_cost: Cost,
    flags: ConsensusFlags,
    timeout: f64,
) -> PyResult<(Cost, LazyNode)> {
    if !(timeout.is_finite() && timeout >= 0.0) {
        return Err(PyValueError::new_err(
            "timeout must be a non-negative finite number of seconds",
        ));
    }
    let timeout = Duration::from_secs_f64(timeout);
    let mut allocator = make_allocator(flags);
    let flags = flags.to_clvm_flags();

    let reduction = (|| -> PyResult<Response> {
        let program = node_from_bytes_backrefs(&mut allocator, program).map_err(map_pyerr)?;
        let args = node_from_bytes_backrefs(&mut allocator, args).map_err(map_pyerr)?;
        let dialect = ChiaDialect::new(flags);

        Ok(py.detach(|| {
            run_program_with_timeout(&mut allocator, &dialect, program, args, max_cost, timeout)
        }))
    })()?
    .map_err(map_pyerr)?;
    let val = LazyNode::new(Rc::new(allocator), reduction.1);
    Ok((reduction.0, val))
}
