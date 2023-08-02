use clvmr::{allocator::NodePtr, reduction::EvalErr};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("{0}")]
    Reason(String),

    #[error("allocator error {0:?}")]
    Allocator(EvalErr),

    #[error("expected atom")]
    ExpectedAtom(NodePtr),

    #[error("expected cons")]
    ExpectedCons(NodePtr),

    #[error("expected nil")]
    ExpectedNil(NodePtr),

    #[error("validation failed")]
    Validation(NodePtr),
}

pub type Result<T> = std::result::Result<T, Error>;
