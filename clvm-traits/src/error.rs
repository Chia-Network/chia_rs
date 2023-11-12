use std::string::FromUtf8Error;

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ToClvmError {
    #[error("limit reached")]
    LimitReached,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum FromClvmError {
    #[error("invalid utf8")]
    InvalidUtf8(#[from] FromUtf8Error),

    #[error("value too large")]
    ValueTooLarge,

    #[error("validation error: {0}")]
    Invalid(String),

    #[error("expected atom")]
    ExpectedAtom,

    #[error("expected pair")]
    ExpectedPair,

    #[error("expected nil")]
    ExpectedNil,
}
