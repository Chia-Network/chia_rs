use clvmr::Allocator;
use clvmr::error::EvalErr;
use clvmr::serde::node_to_bytes;
use pyo3::PyErr;
use pyo3::exceptions::PyValueError;

pub fn map_pyerr(err: EvalErr) -> PyErr {
    PyValueError::new_err(err.to_string())
}
pub fn map_pyerr_w_ptr(err: &EvalErr, alloc: &Allocator) -> PyErr {
    let blob = node_to_bytes(alloc, err.node_ptr()).ok().map(hex::encode);
    PyValueError::new_err((err.to_string(), blob))
}
