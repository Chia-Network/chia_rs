use clvmr::error::EvalErr;
use pyo3::PyErr;
use pyo3::exceptions::PyValueError;

pub fn map_pyerr(err: EvalErr) -> PyErr {
    PyValueError::new_err(err.to_string())
}
