use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::{wrap_pyfunction, PyResult, Python};

use clvm_rs::py::lazy_node::LazyNode;

#[pymodule]
fn chia_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(a_test_function, m)?)?;
    m.add_class::<LazyNode>()?;

    Ok(())
}

#[pyfunction]
pub fn a_test_function() -> u128 {
    500
}
