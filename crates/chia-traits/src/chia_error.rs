use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum Error {
    #[error("invalid bool encoding")]
    InvalidBool,
    #[error("invalid optional encoding")]
    InvalidOptional,
    #[error("unexpected end of buffer")]
    EndOfBuffer,
    #[error("invalid string encoding")]
    InvalidString,
    #[error("input buffer too large")]
    InputTooLarge,
    #[error("sequence too large")]
    SequenceTooLarge,
    #[error("invalid enum value")]
    InvalidEnum,
    #[error("invalid CLVM serialization")]
    InvalidClvm,
    #[error("invalid pot iteration")]
    InvalidPotIteration,
    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "py-bindings")]
impl From<Error> for pyo3::PyErr {
    fn from(err: Error) -> pyo3::PyErr {
        pyo3::exceptions::PyValueError::new_err(err.to_string())
    }
}
