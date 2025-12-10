use clvmr::allocator::{Allocator, Atom, NodePtr, SExp};
use clvmr::error::EvalErr;
use thiserror::Error;

#[cfg(feature = "py-bindings")]
use pyo3::PyErr;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidationErr {
    #[default]
    Unknown,
    InvalidBlockSolution,
    InvalidCoinSolution,
    DuplicateOutput,
    DoubleSpend,
    UnknownUnspent,
    BadAggregateSignature,
    WrongPuzzleHash,
    BadFarmerCoinAmount,
    InvalidCondition(NodePtr),
    InvalidConditionOpcode(NodePtr),
    InvalidParentId(NodePtr),
    InvalidPuzzleHash(NodePtr),
    InvalidPublicKey(NodePtr),
    InvalidMessage(NodePtr),
    InvalidCoinAmount(NodePtr),
    InvalidCoinAnnouncement(NodePtr),
    InvalidPuzzleAnnouncement(NodePtr),
    AssertMyCoinIdFailed(NodePtr),
    AssertPuzzleAnnouncementFailed(NodePtr),
    AssertCoinAnnouncementFailed(NodePtr),
    AssertHeightRelativeFailed(NodePtr),
    AssertHeightAbsoluteFailed(NodePtr),
    AssertSecondsAbsoluteFailed(NodePtr),
    CoinAmountExceedsMaximum(NodePtr),
    SexpError,
    InvalidFeeLowFee,
    MempoolConflict,
    MintingCoin,
    ExtendsUnknownBlock,
    CoinbaseNotYetSpendable,
    /// Renamed from "BlockCostExceedsMax" since it's more generic than that.
    CostExceeded(NodePtr),
    BadAdditionRoot,
    BadRemovalRoot,
    InvalidPospaceHash,
    InvalidCoinbaseSignature,
    InvalidPlotSignature,
    TimestampTooFarInPast,
    TimestampTooFarInFuture,
    InvalidTransactionsFilterHash,
    InvalidPospaceChallenge,
    InvalidPospace,
    InvalidHeight,
    InvalidCoinbaseAmount,
    InvalidMerkleRoot,
    InvalidBlockFeeAmount,
    InvalidWeight,
    InvalidTotalIters,
    BlockIsNotFinished,
    InvalidNumIterations,
    InvalidPot,
    InvalidPotChallenge,
    InvalidTransactionsGeneratorHash,
    InvalidPoolTarget,
    InvalidCoinbaseParent,
    InvalidFeesCoinParent,
    ReserveFeeConditionFailed,
    NotBlockButHasData,
    IsTransactionBlockButNoData,
    InvalidPrevBlockHash,
    InvalidTransactionsInfoHash,
    InvalidFoliageBlockHash,
    InvalidRewardCoins,
    InvalidBlockCost,
    NoEndOfSlotInfo,
    InvalidPrevChallengeSlotHash,
    InvalidSubEpochSummaryHash,
    NoSubEpochSummaryHash,
    ShouldNotMakeChallengeBlock,
    ShouldMakeChallengeBlock,
    InvalidChallengeChainData,
    InvalidCcEosVdf,
    InvalidRcEosVdf,
    InvalidChallengeSlotHashRc,
    InvalidPriorPointRc,
    InvalidDeficit,
    InvalidSubEpochSummary,
    InvalidPrevSubEpochSummaryHash,
    InvalidRewardChainHash,
    InvalidSubEpochOverflow,
    InvalidNewDifficulty,
    InvalidNewSubSlotIters,
    InvalidCcSpVdf,
    InvalidRcSpVdf,
    InvalidCcSignature,
    InvalidRcSignature,
    CannotMakeCcBlock,
    InvalidRcSpPrevIp,
    InvalidRcIpPrevIp,
    InvalidIsTransactionBlock,
    InvalidUrsbHash,
    OldPoolTarget,
    InvalidPoolSignature,
    InvalidFoliageBlockPresence,
    InvalidCcIpVdf,
    InvalidRcIpVdf,
    IpShouldBeNone,
    InvalidRewardBlockHash,
    InvalidMadeNonOverflowInfusions,
    NoOverflowsInFirstSubSlotNewEpoch,
    MempoolNotInitialized,
    ShouldNotHaveIcc,
    ShouldHaveIcc,
    InvalidIccVdf,
    InvalidIccHashCc,
    InvalidIccHashRc,
    InvalidIccEosVdf,
    InvalidSpIndex,
    TooManyBlocks,
    InvalidCcChallenge,
    InvalidPrefarm,
    AssertSecondsRelativeFailed,
    BadCoinbaseSignature,
    // InitialTransactionFreeze (removed in `chia-blockchain` as well)
    NoTransactionsWhileSyncing,
    AlreadyIncludingTransaction,
    IncompatibleNetworkId,
    PreSoftForkMaxGeneratorSize,
    InvalidRequiredIters,
    TooManyGeneratorRefs,
    AssertMyParentIdFailed(NodePtr),
    AssertMyPuzzleHashFailed(NodePtr),
    AssertMyAmountFailed(NodePtr),
    GeneratorRuntimeError(NodePtr),
    InvalidCostResult,
    InvalidTransactionsGeneratorRefsRoot,
    FutureGeneratorRefs,
    GeneratorRefHasNoGenerator,
    DoubleSpendInFork,
    InvalidFeeTooCloseToZero,
    CoinAmountNegative,
    InternalProtocolError,
    InvalidSpendBundle,
    FailedGettingGeneratorMultiprocessing,
    AssertBeforeSecondsAbsoluteFailed,
    AssertBeforeSecondsRelativeFailed,
    AssertBeforeHeightAbsoluteFailed,
    AssertBeforeHeightRelativeFailed,
    AssertConcurrentSpendFailed,
    AssertConcurrentPuzzleFailed,
    ImpossibleSecondsRelativeConstraints,
    ImpossibleSecondsAbsoluteConstraints,
    ImpossibleHeightRelativeConstraints,
    ImpossibleHeightAbsoluteConstraints,
    AssertMyBirthSecondsFailed,
    AssertMyBirthHeightFailed,
    AssertEphemeralFailed,
    EphemeralRelativeCondition,
    InvalidSoftforkCondition,
    InvalidSoftforkCost,
    TooManyAnnouncements,
    InvalidMessageMode(NodePtr),
    InvalidCoinId,
    MessageNotSentOrReceived,
    ComplexGeneratorReceived,
}

impl From<EvalErr> for ValidationErr {
    fn from(v: EvalErr) -> Self {
        match v {
            EvalErr::CostExceeded => ValidationErr::CostExceeded(v.node_ptr()),
            _ => ValidationErr::GeneratorRuntimeError(v.node_ptr()),
        }
    }
}

impl From<std::io::Error> for ValidationErr {
    fn from(_: std::io::Error) -> Self {
        ValidationErr::GeneratorRuntimeError(NodePtr::NIL)
    }
}

#[cfg(feature = "py-bindings")]
impl From<ValidationErr> for PyErr {
    fn from(err: ValidationErr) -> PyErr {
        pyo3::exceptions::PyValueError::new_err(("ValidationError", u32::from(err)))
    }
}

// helper functions that fail with ValidationErr
pub fn first(a: &Allocator, n: NodePtr) -> Result<NodePtr, ValidationErr> {
    match a.sexp(n) {
        SExp::Pair(left, _) => Ok(left),
        SExp::Atom => Err(ValidationErr::InvalidCondition(n)),
    }
}

// from chia-blockchain/chia/util/errors.py
impl From<ValidationErr> for u32 {
    fn from(err: ValidationErr) -> u32 {
        match err {
            ValidationErr::Unknown => 1,
            ValidationErr::InvalidBlockSolution => 2,
            ValidationErr::InvalidCoinSolution => 3,
            ValidationErr::DuplicateOutput => 4,
            ValidationErr::DoubleSpend => 5,
            ValidationErr::UnknownUnspent => 6,
            ValidationErr::BadAggregateSignature => 7,
            ValidationErr::WrongPuzzleHash => 8,
            ValidationErr::BadFarmerCoinAmount => 9,
            ValidationErr::InvalidCondition
            | ValidationErr::InvalidConditionOpcode
            | ValidationErr::InvalidParentId
            | ValidationErr::InvalidPuzzleHash
            | ValidationErr::InvalidPublicKey
            | ValidationErr::InvalidMessage
            | ValidationErr::InvalidCoinAmount
            | ValidationErr::InvalidCoinAnnouncement
            | ValidationErr::InvalidPuzzleAnnouncement => 10,
            ValidationErr::AssertMyCoinIdFailed => 11,
            ValidationErr::AssertPuzzleAnnouncementFailed
            | ValidationErr::AssertCoinAnnouncementFailed => 12,
            ValidationErr::AssertHeightRelativeFailed => 13,
            ValidationErr::AssertHeightAbsoluteFailed => 14,
            ValidationErr::AssertSecondsAbsoluteFailed => 15,
            ValidationErr::CoinAmountExceedsMaximum => 16,
            ValidationErr::SexpError => 17,
            ValidationErr::InvalidFeeLowFee => 18,
            ValidationErr::MempoolConflict => 19,
            ValidationErr::MintingCoin => 20,
            ValidationErr::ExtendsUnknownBlock => 21,
            ValidationErr::CoinbaseNotYetSpendable => 22,
            ValidationErr::CostExceeded => 23,
            ValidationErr::BadAdditionRoot => 24,
            ValidationErr::BadRemovalRoot => 25,
            ValidationErr::InvalidPospaceHash => 26,
            ValidationErr::InvalidCoinbaseSignature => 27,
            ValidationErr::InvalidPlotSignature => 28,
            ValidationErr::TimestampTooFarInPast => 29,
            ValidationErr::TimestampTooFarInFuture => 30,
            ValidationErr::InvalidTransactionsFilterHash => 31,
            ValidationErr::InvalidPospaceChallenge => 32,
            ValidationErr::InvalidPospace => 33,
            ValidationErr::InvalidHeight => 34,
            ValidationErr::InvalidCoinbaseAmount => 35,
            ValidationErr::InvalidMerkleRoot => 36,
            ValidationErr::InvalidBlockFeeAmount => 37,
            ValidationErr::InvalidWeight => 38,
            ValidationErr::InvalidTotalIters => 39,
            ValidationErr::BlockIsNotFinished => 40,
            ValidationErr::InvalidNumIterations => 41,
            ValidationErr::InvalidPot => 42,
            ValidationErr::InvalidPotChallenge => 43,
            ValidationErr::InvalidTransactionsGeneratorHash => 44,
            ValidationErr::InvalidPoolTarget => 45,
            ValidationErr::InvalidCoinbaseParent => 46,
            ValidationErr::InvalidFeesCoinParent => 47,
            ValidationErr::ReserveFeeConditionFailed => 48,
            ValidationErr::NotBlockButHasData => 49,
            ValidationErr::IsTransactionBlockButNoData => 50,
            ValidationErr::InvalidPrevBlockHash => 51,
            ValidationErr::InvalidTransactionsInfoHash => 52,
            ValidationErr::InvalidFoliageBlockHash => 53,
            ValidationErr::InvalidRewardCoins => 54,
            ValidationErr::InvalidBlockCost => 55,
            ValidationErr::NoEndOfSlotInfo => 56,
            ValidationErr::InvalidPrevChallengeSlotHash => 57,
            ValidationErr::InvalidSubEpochSummaryHash => 58,
            ValidationErr::NoSubEpochSummaryHash => 59,
            ValidationErr::ShouldNotMakeChallengeBlock => 60,
            ValidationErr::ShouldMakeChallengeBlock => 61,
            ValidationErr::InvalidChallengeChainData => 62,
            ValidationErr::InvalidCcEosVdf => 65,
            ValidationErr::InvalidRcEosVdf => 66,
            ValidationErr::InvalidChallengeSlotHashRc => 67,
            ValidationErr::InvalidPriorPointRc => 68,
            ValidationErr::InvalidDeficit => 69,
            ValidationErr::InvalidSubEpochSummary => 70,
            ValidationErr::InvalidPrevSubEpochSummaryHash => 71,
            ValidationErr::InvalidRewardChainHash => 72,
            ValidationErr::InvalidSubEpochOverflow => 73,
            ValidationErr::InvalidNewDifficulty => 74,
            ValidationErr::InvalidNewSubSlotIters => 75,
            ValidationErr::InvalidCcSpVdf => 76,
            ValidationErr::InvalidRcSpVdf => 77,
            ValidationErr::InvalidCcSignature => 78,
            ValidationErr::InvalidRcSignature => 79,
            ValidationErr::CannotMakeCcBlock => 80,
            ValidationErr::InvalidRcSpPrevIp => 81,
            ValidationErr::InvalidRcIpPrevIp => 82,
            ValidationErr::InvalidIsTransactionBlock => 83,
            ValidationErr::InvalidUrsbHash => 84,
            ValidationErr::OldPoolTarget => 85,
            ValidationErr::InvalidPoolSignature => 86,
            ValidationErr::InvalidFoliageBlockPresence => 87,
            ValidationErr::InvalidCcIpVdf => 88,
            ValidationErr::InvalidRcIpVdf => 89,
            ValidationErr::IpShouldBeNone => 90,
            ValidationErr::InvalidRewardBlockHash => 91,
            ValidationErr::InvalidMadeNonOverflowInfusions => 92,
            ValidationErr::NoOverflowsInFirstSubSlotNewEpoch => 93,
            ValidationErr::MempoolNotInitialized => 94,
            ValidationErr::ShouldNotHaveIcc => 95,
            ValidationErr::ShouldHaveIcc => 96,
            ValidationErr::InvalidIccVdf => 97,
            ValidationErr::InvalidIccHashCc => 98,
            ValidationErr::InvalidIccHashRc => 99,
            ValidationErr::InvalidIccEosVdf => 100,
            ValidationErr::InvalidSpIndex => 101,
            ValidationErr::TooManyBlocks => 102,
            ValidationErr::InvalidCcChallenge => 103,
            ValidationErr::InvalidPrefarm => 104,
            ValidationErr::AssertSecondsRelativeFailed => 105,
            ValidationErr::BadCoinbaseSignature => 106,
            // ValidationErr::InitialTransactionFreeze => 107 (removed in `chia-blockchain`` as well)
            ValidationErr::NoTransactionsWhileSyncing => 108,
            ValidationErr::AlreadyIncludingTransaction => 109,
            ValidationErr::IncompatibleNetworkId => 110,
            ValidationErr::PreSoftForkMaxGeneratorSize => 111,
            ValidationErr::InvalidRequiredIters => 112,
            ValidationErr::TooManyGeneratorRefs => 113,
            ValidationErr::AssertMyParentIdFailed => 114,
            ValidationErr::AssertMyPuzzleHashFailed => 115,
            ValidationErr::AssertMyAmountFailed => 116,
            ValidationErr::GeneratorRuntimeError => 117,
            ValidationErr::InvalidCostResult => 118,
            ValidationErr::InvalidTransactionsGeneratorRefsRoot => 119,
            ValidationErr::FutureGeneratorRefs => 120,
            ValidationErr::GeneratorRefHasNoGenerator => 121,
            ValidationErr::DoubleSpendInFork => 122,
            ValidationErr::InvalidFeeTooCloseToZero => 123,
            ValidationErr::CoinAmountNegative => 124,
            ValidationErr::InternalProtocolError => 125,
            ValidationErr::InvalidSpendBundle => 126,
            ValidationErr::FailedGettingGeneratorMultiprocessing => 127,
            ValidationErr::AssertBeforeSecondsAbsoluteFailed => 128,
            ValidationErr::AssertBeforeSecondsRelativeFailed => 129,
            ValidationErr::AssertBeforeHeightAbsoluteFailed => 130,
            ValidationErr::AssertBeforeHeightRelativeFailed => 131,
            ValidationErr::AssertConcurrentSpendFailed => 132,
            ValidationErr::AssertConcurrentPuzzleFailed => 133,
            ValidationErr::ImpossibleSecondsRelativeConstraints => 134,
            ValidationErr::ImpossibleSecondsAbsoluteConstraints => 135,
            ValidationErr::ImpossibleHeightRelativeConstraints => 136,
            ValidationErr::ImpossibleHeightAbsoluteConstraints => 137,
            ValidationErr::AssertMyBirthSecondsFailed => 138,
            ValidationErr::AssertMyBirthHeightFailed => 139,
            ValidationErr::AssertEphemeralFailed => 140,
            ValidationErr::EphemeralRelativeCondition => 141,
            ValidationErr::InvalidSoftforkCondition => 142,
            ValidationErr::InvalidSoftforkCost => 143,
            ValidationErr::TooManyAnnouncements => 144,
            ValidationErr::InvalidMessageMode => 145,
            ValidationErr::InvalidCoinId => 146,
            ValidationErr::MessageNotSentOrReceived => 147,
            ValidationErr::ComplexGeneratorReceived => 148,
        }
    }
}

pub fn rest(a: &Allocator, n: NodePtr) -> Result<NodePtr, ValidationErr> {
    match a.sexp(n) {
        SExp::Pair(_, right) => Ok(right),
        SExp::Atom => Err(ValidationErr::InvalidCondition(n)),
    }
}

pub fn next(a: &Allocator, n: NodePtr) -> Result<Option<(NodePtr, NodePtr)>, ValidationErr> {
    match a.sexp(n) {
        SExp::Pair(left, right) => Ok(Some((left, right))),
        SExp::Atom => {
            // this is expected to be a valid list terminator
            if a.atom_len(n) == 0 {
                Ok(None)
            } else {
                Err(ValidationErr::InvalidCondition(n))
            }
        }
    }
}

pub fn atom(a: &Allocator, n: NodePtr, code: ValidationErr) -> Result<Atom<'_>, ValidationErr> {
    match a.sexp(n) {
        SExp::Atom => Ok(a.atom(n)),
        SExp::Pair(..) => Err(code),
    }
}

pub fn check_nil(a: &Allocator, n: NodePtr) -> Result<(), ValidationErr> {
    if atom(a, n, ValidationErr::InvalidCondition)?
        .as_ref()
        .is_empty()
    {
        Ok(())
    } else {
        Err(ValidationErr::InvalidCondition(n))
    }
    d
}
