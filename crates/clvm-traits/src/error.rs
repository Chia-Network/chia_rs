use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ToClvmError {
    #[error("out of memory")]
    OutOfMemory,

    #[error("{0}")]
    Custom(String),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum FromClvmError {
    #[error("{0}")]
    InvalidUtf8(#[from] FromUtf8Error),

    #[error("expected atom of length {expected}, but found length {found}")]
    WrongAtomLength { expected: usize, found: usize },

    #[error("expected atom")]
    ExpectedAtom,

    #[error("expected pair")]
    ExpectedPair,

    #[error("{0}")]
    Custom(String),
}

#[cfg(feature = "py-bindings")]
use pyo3::PyErr;

#[cfg(feature = "py-bindings")]
impl From<ToClvmError> for PyErr {
    fn from(err: ToClvmError) -> PyErr {
        pyo3::exceptions::PyValueError::new_err(err.to_string())
    }
}

#[cfg(feature = "py-bindings")]
impl From<FromClvmError> for PyErr {
    fn from(err: FromClvmError) -> PyErr {
        pyo3::exceptions::PyValueError::new_err(err.to_string())
    }
}
