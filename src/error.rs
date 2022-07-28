use std::fmt;
use std::io;

#[cfg(feature = "py-bindings")]
use pyo3::exceptions;
#[cfg(feature = "py-bindings")]
use pyo3::PyErr;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    NotSupported,
    InvalidBool,
    InvalidOptional,
    EndOfBuffer,
    InvalidString,
    InputTooLarge,
    SequenceTooLarge,
    Custom(String),
}

#[cfg(test)]
impl PartialEq for Error {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (Error::Io(a), Error::Io(b)) => a.to_string() == b.to_string(),
            (Error::NotSupported, Error::NotSupported) => true,
            (Error::InvalidBool, Error::InvalidBool) => true,
            (Error::InvalidOptional, Error::InvalidOptional) => true,
            (Error::EndOfBuffer, Error::EndOfBuffer) => true,
            (Error::InvalidString, Error::InvalidString) => true,
            (Error::InputTooLarge, Error::InputTooLarge) => true,
            (Error::SequenceTooLarge, Error::SequenceTooLarge) => true,
            (Error::Custom(a), Error::Custom(b)) => a == b,
            (_, _) => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(desc: T) -> Error {
        Error::Custom(desc.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::Io(ref err) => Some(err),
            Error::NotSupported => None,
            Error::InvalidBool => None,
            Error::InvalidOptional => None,
            Error::InvalidString => None,
            Error::EndOfBuffer => None,
            Error::InputTooLarge => None,
            Error::SequenceTooLarge => None,
            Error::Custom(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref ioerr) => write!(fmt, "io error: {}", ioerr),
            Error::NotSupported => write!(fmt, "not supported"),
            Error::InvalidBool => write!(fmt, "invalid bool encoding"),
            Error::InvalidOptional => write!(fmt, "invalid optional encoding"),
            Error::InvalidString => write!(fmt, "invalid string encoding"),
            Error::EndOfBuffer => write!(fmt, "unexpected end of buffer"),
            Error::InputTooLarge => write!(fmt, "input buffer too large"),
            Error::SequenceTooLarge => write!(fmt, "sequence too large"),
            Error::Custom(ref s) => s.fmt(fmt),
        }
    }
}

#[cfg(feature = "py-bindings")]
impl std::convert::From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        exceptions::PyValueError::new_err(err.to_string())
    }
}
