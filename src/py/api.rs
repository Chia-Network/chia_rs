use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::{wrap_pyfunction, PyResult, Python};

use clvm_rs::py::lazy_node::LazyNode;

use crate::py::run_generator::__pyo3_get_function_run_generator2;

pub fn chia_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(a_test_function, m)?)?;
    m.add_function(wrap_pyfunction!(run_generator2, m)?)?;
    m.add_class::<LazyNode>()?;

    Ok(())
}

#[pyfunction]
pub fn a_test_function() -> u128 {
    500
}
