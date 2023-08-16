use clvmr::{allocator::NodePtr, reduction::EvalErr};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("allocator error {0:?}")]
    Allocator(EvalErr),

    #[error("expected atom")]
    ExpectedAtom(NodePtr),

    #[error("expected cons")]
    ExpectedCons(NodePtr),

    #[error("expected nil")]
    ExpectedNil(NodePtr),

    #[error("expected one")]
    ExpectedOne(NodePtr),

    #[error("validation failed")]
    Validation(NodePtr),

    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<EvalErr> for Error {
    fn from(value: EvalErr) -> Self {
        Self::Allocator(value)
    }
}
