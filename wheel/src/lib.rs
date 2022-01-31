use pyo3::prelude::pymodule;
use pyo3::types::PyModule;
use pyo3::{PyResult, Python};

use chia::py::api::chia_rs as py_chia_rs;

#[pymodule]
fn chia_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    py_chia_rs(_py, m)
}
