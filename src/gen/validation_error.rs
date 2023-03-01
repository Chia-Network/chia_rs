use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::reduction::EvalErr;

#[cfg(feature = "py-bindings")]
use pyo3::exceptions;
#[cfg(feature = "py-bindings")]
use pyo3::PyErr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    GeneratorRuntimeError,
    NegativeAmount,
    AmountExceedsMaximum,
    InvalidConditionOpcode,
    InvalidParentId,
    InvalidPuzzleHash,
    InvalidPubkey,
    InvalidMessage,
    InvalidCondition,
    InvalidCoinAmount,
    InvalidCoinAnnouncement,
    InvalidPuzzleAnnouncement,
    AssertHeightAbsolute,
    AssertHeightRelative,
    AssertBeforeSecondsAbsolute,
    AssertBeforeSecondsRelative,
    AssertBeforeHeightAbsolute,
    AssertBeforeHeightRelative,
    AssertSecondsAbsolute,
    AssertSecondsRelative,
    AssertMyAmountFailed,
    AssertMyBirthSecondsFailed,
    AssertMyBirthHeightFailed,
    AssertMyPuzzlehashFailed,
    AssertMyParentIdFailed,
    AssertMyCoinIdFailed,
    AssertPuzzleAnnouncementFailed,
    AssertCoinAnnouncementFailed,
    AssertConcurrentSpendFailed,
    AssertConcurrentPuzzleFailed,
    ReserveFeeConditionFailed,
    DuplicateOutput,
    DoubleSpend,
    CostExceeded,
    MintingCoin,
    ImpossibleSecondsRelativeConstraints,
    ImpossibleSecondsAbsoluteConstraints,
    ImpossibleHeightRelativeConstraints,
    ImpossibleHeightAbsoluteConstraints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationErr(pub NodePtr, pub ErrorCode);

impl From<EvalErr> for ValidationErr {
    fn from(v: EvalErr) -> Self {
        ValidationErr(v.0, ErrorCode::GeneratorRuntimeError)
    }
}

impl From<std::io::Error> for ValidationErr {
    fn from(_: std::io::Error) -> Self {
        ValidationErr(-1, ErrorCode::GeneratorRuntimeError)
    }
}

#[cfg(feature = "py-bindings")]
impl std::convert::From<ValidationErr> for PyErr {
    fn from(err: ValidationErr) -> PyErr {
        exceptions::PyValueError::new_err(("ValidationError", u32::from(err.1)))
    }
}

// helper functions that fail with ValidationErr
pub fn first(a: &Allocator, n: NodePtr) -> Result<NodePtr, ValidationErr> {
    match a.sexp(n) {
        SExp::Pair(left, _) => Ok(left),
        _ => Err(ValidationErr(n, ErrorCode::InvalidCondition)),
    }
}

// from chia-blockchain/chia/util/errors.py
impl From<ErrorCode> for u32 {
    fn from(err: ErrorCode) -> u32 {
        match err {
            ErrorCode::GeneratorRuntimeError => 117,
            ErrorCode::NegativeAmount => 124,
            ErrorCode::AmountExceedsMaximum => 16,
            ErrorCode::InvalidPuzzleHash => 10,
            ErrorCode::InvalidPubkey => 10,
            ErrorCode::InvalidMessage => 10,
            ErrorCode::InvalidParentId => 10,
            ErrorCode::InvalidConditionOpcode => 10,
            ErrorCode::InvalidCoinAnnouncement => 10,
            ErrorCode::InvalidPuzzleAnnouncement => 10,
            ErrorCode::InvalidCondition => 10,
            ErrorCode::InvalidCoinAmount => 10,
            ErrorCode::AssertHeightAbsolute => 14,
            ErrorCode::AssertHeightRelative => 13,
            ErrorCode::AssertBeforeSecondsAbsolute => 128,
            ErrorCode::AssertBeforeSecondsRelative => 129,
            ErrorCode::AssertBeforeHeightAbsolute => 130,
            ErrorCode::AssertBeforeHeightRelative => 131,
            ErrorCode::AssertSecondsAbsolute => 15,
            ErrorCode::AssertSecondsRelative => 105,
            ErrorCode::AssertMyAmountFailed => 116,
            ErrorCode::AssertMyPuzzlehashFailed => 115,
            ErrorCode::AssertMyParentIdFailed => 114,
            ErrorCode::AssertMyCoinIdFailed => 11,
            ErrorCode::AssertPuzzleAnnouncementFailed => 12,
            ErrorCode::AssertCoinAnnouncementFailed => 12,
            ErrorCode::AssertConcurrentSpendFailed => 132,
            ErrorCode::AssertConcurrentPuzzleFailed => 133,
            ErrorCode::ReserveFeeConditionFailed => 48,
            ErrorCode::DuplicateOutput => 4,
            ErrorCode::DoubleSpend => 5,
            ErrorCode::CostExceeded => 23,
            ErrorCode::MintingCoin => 20,
            ErrorCode::ImpossibleSecondsRelativeConstraints => 134,
            ErrorCode::ImpossibleSecondsAbsoluteConstraints => 135,
            ErrorCode::ImpossibleHeightRelativeConstraints => 136,
            ErrorCode::ImpossibleHeightAbsoluteConstraints => 137,
            ErrorCode::AssertMyBirthSecondsFailed => 138,
            ErrorCode::AssertMyBirthHeightFailed => 139,
        }
    }
}

pub fn rest(a: &Allocator, n: NodePtr) -> Result<NodePtr, ValidationErr> {
    match a.sexp(n) {
        SExp::Pair(_, right) => Ok(right),
        _ => Err(ValidationErr(n, ErrorCode::InvalidCondition)),
    }
}

pub fn next(a: &Allocator, n: NodePtr) -> Result<Option<(NodePtr, NodePtr)>, ValidationErr> {
    match a.sexp(n) {
        SExp::Pair(left, right) => Ok(Some((left, right))),
        SExp::Atom(v) => {
            // this is expected to be a valid list terminator
            if v.is_empty() {
                Ok(None)
            } else {
                Err(ValidationErr(n, ErrorCode::InvalidCondition))
            }
        }
    }
}

pub fn atom(a: &Allocator, n: NodePtr, code: ErrorCode) -> Result<&[u8], ValidationErr> {
    match a.sexp(n) {
        SExp::Atom(_) => Ok(a.atom(n)),
        _ => Err(ValidationErr(n, code)),
    }
}

pub fn check_nil(a: &Allocator, n: NodePtr) -> Result<(), ValidationErr> {
    if atom(a, n, ErrorCode::InvalidCondition)?.is_empty() {
        Ok(())
    } else {
        Err(ValidationErr(n, ErrorCode::InvalidCondition))
    }
}
