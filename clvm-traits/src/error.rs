use std::string::FromUtf8Error;

use thiserror::Error;

/// Any errors that may occur while converting a Rust value to a CLVM value.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ToClvmError {
    #[error("limit reached")]
    LimitReached,
}

/// Any errors that may occur while converting a CLVM value to a Rust value.
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
