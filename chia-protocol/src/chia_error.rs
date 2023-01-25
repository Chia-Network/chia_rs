use std::error;
use std::fmt;

#[cfg(feature = "py-bindings")]
use pyo3::exceptions;
#[cfg(feature = "py-bindings")]
use pyo3::PyErr;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    InvalidBool,
    InvalidOptional,
    EndOfBuffer,
    InvalidString,
    InputTooLarge,
    SequenceTooLarge,
    InvalidEnum,
    Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::InvalidBool => write!(fmt, "invalid bool encoding"),
            Error::InvalidOptional => write!(fmt, "invalid optional encoding"),
            Error::InvalidString => write!(fmt, "invalid string encoding"),
            Error::EndOfBuffer => write!(fmt, "unexpected end of buffer"),
            Error::InputTooLarge => write!(fmt, "input buffer too large"),
            Error::SequenceTooLarge => write!(fmt, "sequence too large"),
            Error::InvalidEnum => write!(fmt, "invalid enum value"),
            Error::Custom(ref s) => s.fmt(fmt),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

#[cfg(feature = "py-bindings")]
impl std::convert::From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        exceptions::PyValueError::new_err(err.to_string())
    }
}
