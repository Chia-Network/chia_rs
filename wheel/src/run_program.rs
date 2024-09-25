use chia_consensus::allocator::make_allocator;
use chia_consensus::gen::flags::ALLOW_BACKREFS;
use chia_protocol::LazyNode;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Response;
use clvmr::run_program::run_program;
use clvmr::serde::{
    node_from_bytes, node_from_bytes_backrefs, node_to_bytes, serialized_length_from_bytes,
};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::rc::Rc;

#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn serialized_length(program: PyBuffer<u8>) -> PyResult<u64> {
    assert!(program.is_c_contiguous(), "program must be contiguous");
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };
    Ok(serialized_length_from_bytes(program)?)
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
        let deserialize = if (flags & ALLOW_BACKREFS) != 0 {
            node_from_bytes_backrefs
        } else {
            node_from_bytes
        };
        let program = deserialize(&mut allocator, program)?;
        let args = deserialize(&mut allocator, args)?;
        let dialect = ChiaDialect::new(flags);

        Ok(py.allow_threads(|| run_program(&mut allocator, &dialect, program, args, max_cost)))
    })()?
    .map_err(|e| {
        let blob = node_to_bytes(&allocator, e.0).ok().map(hex::encode);
        PyValueError::new_err((e.1, blob))
    })?;
    let val = LazyNode::new(Rc::new(allocator), reduction.1);
    Ok((reduction.0, val))
}
