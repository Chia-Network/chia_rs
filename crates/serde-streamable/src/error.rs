use std::{fmt, str::Utf8Error, string::FromUtf8Error};

use serde::{de, ser};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("Sequences with unknown length are not supported")]
    UnknownLength,

    #[error("Enums are not supported")]
    Enum,

    #[error("Maps are not supported")]
    Map,

    #[error("Identifiers are not supported")]
    Identifier,

    #[error("Binary formats do not generally support `deserialize_any`")]
    DeserializeAny,

    #[error("Expected end of input")]
    ExpectedEof,

    #[error("Unexpected end of input")]
    UnexpectedEof,

    #[error("Unexpected boolean byte {0}")]
    UnexpectedBool(u8),

    #[error("Unexpected integer byte for optional {0}")]
    UnexpectedOptionalInt(u8),

    #[error("Invalid UTF-8 for String")]
    Utf8String(#[from] FromUtf8Error),

    #[error("Invalid UTF-8 for str")]
    Utf8Str(#[from] Utf8Error),

    #[error("Missing char")]
    MissingChar,

    #[error("{0}")]
    Custom(String),
}

impl ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
