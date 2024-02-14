use clvmr::allocator::Allocator;
use clvmr::reduction::EvalErr;
use clvmr::serde::node_to_bytes;

use pyo3::exceptions::*;
use pyo3::prelude::*;

pub fn eval_err_to_pyresult<T>(eval_err: EvalErr, allocator: Allocator) -> PyResult<T> {
    let blob = node_to_bytes(&allocator, eval_err.0).ok().map(hex::encode);
    Err(PyValueError::new_err((eval_err.1, blob)))
}
