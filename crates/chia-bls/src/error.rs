use blst::BLST_ERROR;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("SecretKey byte data must be less than the group order")]
    SecretKeyGroupOrder,
    #[error("Given G1 infinity element must be canonical")]
    G1NotCanonical,
    #[error("Given G1 non-infinity element must start with 0b10")]
    G1InfinityInvalidBits,
    #[error("G1 non-infinity element can't have only zeros")]
    G1InfinityNotZero,
    #[error("PublicKey is invalid (BLST ERROR: {0:?})")]
    InvalidPublicKey(BLST_ERROR),
    #[error("Signature is invalid (BLST ERROR: {0:?})")]
    InvalidSignature(BLST_ERROR),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for chia_traits::Error {
    fn from(err: Error) -> chia_traits::Error {
        chia_traits::Error::Custom(format!("{err}"))
    }
}

#[cfg(feature = "py-bindings")]
mod pybindings {
    use super::*;

    use pyo3::{exceptions::PyValueError, PyErr};

    impl From<Error> for PyErr {
        fn from(err: Error) -> PyErr {
            PyValueError::new_err(format!("BLS Error {err:?}"))
        }
    }
}
