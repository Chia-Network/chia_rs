use chia::compression::compressor::create_autoextracting_clvm_program;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyModule;
use pyo3::{wrap_pyfunction, PyResult, Python};

#[pyfunction]
fn create_compressed_generator<'p>(py: Python<'p>, input_program: &[u8]) -> PyResult<&'p PyBytes> {
    let vec: Vec<u8> = create_autoextracting_clvm_program(input_program)?;
    Ok(PyBytes::new(py, &vec))
}

pub fn add_submodule(py: Python, m: &PyModule) -> PyResult<()> {
    let submod = PyModule::new(py, "compression")?;
    submod.add_function(wrap_pyfunction!(create_compressed_generator, m)?)?;
    m.add_submodule(submod)
}
