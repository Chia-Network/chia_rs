use clvmr::allocator::{Allocator, Atom, NodePtr, SExp};
use clvmr::error::EvalErr;

#[cfg(feature = "py-bindings")]
use pyo3::PyErr;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorCode {
    #[default]
    Unknown,
    InvalidBlockSolution(NodePtr),
    InvalidCoinSolution(NodePtr),
    DuplicateOutput(NodePtr),
    DoubleSpend(NodePtr),
    UnknownUnspent(NodePtr),
    BadAggregateSignature,
    WrongPuzzleHash(NodePtr),
    BadFarmerCoinAmount(NodePtr),
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
    SexpError(NodePtr),
    InvalidFeeLowFee(NodePtr),
    MempoolConflict(NodePtr),
    MintingCoin(NodePtr),
    ExtendsUnknownBlock(NodePtr),
    CoinbaseNotYetSpendable(NodePtr),
    /// Renamed from "BlockCostExceedsMax" since it's more generic than that.
    CostExceeded(Option<NodePtr>),
    BadAdditionRoot(NodePtr),
    BadRemovalRoot(NodePtr),
    InvalidPospaceHash(NodePtr),
    InvalidCoinbaseSignature(NodePtr),
    InvalidPlotSignature(NodePtr),
    TimestampTooFarInPast(NodePtr),
    TimestampTooFarInFuture(NodePtr),
    InvalidTransactionsFilterHash(NodePtr),
    InvalidPospaceChallenge(NodePtr),
    InvalidPospace(NodePtr),
    InvalidHeight(NodePtr),
    InvalidCoinbaseAmount(NodePtr),
    InvalidMerkleRoot(NodePtr),
    InvalidBlockFeeAmount(NodePtr),
    InvalidWeight(NodePtr),
    InvalidTotalIters(NodePtr),
    BlockIsNotFinished(NodePtr),
    InvalidNumIterations(NodePtr),
    InvalidPot(NodePtr),
    InvalidPotChallenge(NodePtr),
    InvalidTransactionsGeneratorHash(NodePtr),
    InvalidPoolTarget(NodePtr),
    InvalidCoinbaseParent(NodePtr),
    InvalidFeesCoinParent(NodePtr),
    ReserveFeeConditionFailed(NodePtr),
    NotBlockButHasData(NodePtr),
    IsTransactionBlockButNoData(NodePtr),
    InvalidPrevBlockHash(NodePtr),
    InvalidTransactionsInfoHash(NodePtr),
    InvalidFoliageBlockHash(NodePtr),
    InvalidRewardCoins(NodePtr),
    InvalidBlockCost(NodePtr),
    NoEndOfSlotInfo(NodePtr),
    InvalidPrevChallengeSlotHash(NodePtr),
    InvalidSubEpochSummaryHash(NodePtr),
    NoSubEpochSummaryHash(NodePtr),
    ShouldNotMakeChallengeBlock(NodePtr),
    ShouldMakeChallengeBlock(NodePtr),
    InvalidChallengeChainData(NodePtr),
    InvalidCcEosVdf(NodePtr),
    InvalidRcEosVdf(NodePtr),
    InvalidChallengeSlotHashRc(NodePtr),
    InvalidPriorPointRc(NodePtr),
    InvalidDeficit(NodePtr),
    InvalidSubEpochSummary(NodePtr),
    InvalidPrevSubEpochSummaryHash(NodePtr),
    InvalidRewardChainHash(NodePtr),
    InvalidSubEpochOverflow(NodePtr),
    InvalidNewDifficulty(NodePtr),
    InvalidNewSubSlotIters(NodePtr),
    InvalidCcSpVdf(NodePtr),
    InvalidRcSpVdf(NodePtr),
    InvalidCcSignature(NodePtr),
    InvalidRcSignature(NodePtr),
    CannotMakeCcBlock(NodePtr),
    InvalidRcSpPrevIp(NodePtr),
    InvalidRcIpPrevIp(NodePtr),
    InvalidIsTransactionBlock(NodePtr),
    InvalidUrsbHash(NodePtr),
    OldPoolTarget(NodePtr),
    InvalidPoolSignature(NodePtr),
    InvalidFoliageBlockPresence(NodePtr),
    InvalidCcIpVdf(NodePtr),
    InvalidRcIpVdf(NodePtr),
    IpShouldBeNone(NodePtr),
    InvalidRewardBlockHash(NodePtr),
    InvalidMadeNonOverflowInfusions(NodePtr),
    NoOverflowsInFirstSubSlotNewEpoch(NodePtr),
    MempoolNotInitialized(NodePtr),
    ShouldNotHaveIcc(NodePtr),
    ShouldHaveIcc(NodePtr),
    InvalidIccVdf(NodePtr),
    InvalidIccHashCc(NodePtr),
    InvalidIccHashRc(NodePtr),
    InvalidIccEosVdf(NodePtr),
    InvalidSpIndex(NodePtr),
    TooManyBlocks(NodePtr),
    InvalidCcChallenge(NodePtr),
    InvalidPrefarm(NodePtr),
    AssertSecondsRelativeFailed(NodePtr),
    BadCoinbaseSignature(NodePtr),
    // InitialTransactionFreeze (removed in `chia-blockchain` as well)
    NoTransactionsWhileSyncing(NodePtr),
    AlreadyIncludingTransaction(NodePtr),
    IncompatibleNetworkId(NodePtr),
    PreSoftForkMaxGeneratorSize(NodePtr),
    InvalidRequiredIters(NodePtr),
    TooManyGeneratorRefs,
    AssertMyParentIdFailed(NodePtr),
    AssertMyPuzzleHashFailed(NodePtr),
    AssertMyAmountFailed(NodePtr),
    GeneratorRuntimeError(Option<NodePtr>),
    InvalidCostResult(NodePtr),
    InvalidTransactionsGeneratorRefsRoot(NodePtr),
    FutureGeneratorRefs(NodePtr),
    GeneratorRefHasNoGenerator(NodePtr),
    DoubleSpendInFork(NodePtr),
    InvalidFeeTooCloseToZero(NodePtr),
    CoinAmountNegative(NodePtr),
    InternalProtocolError(NodePtr),
    InvalidSpendBundle(NodePtr),
    FailedGettingGeneratorMultiprocessing(NodePtr),
    AssertBeforeSecondsAbsoluteFailed(NodePtr),
    AssertBeforeSecondsRelativeFailed(NodePtr),
    AssertBeforeHeightAbsoluteFailed(NodePtr),
    AssertBeforeHeightRelativeFailed(NodePtr),
    AssertConcurrentSpendFailed(NodePtr),
    AssertConcurrentPuzzleFailed(NodePtr),
    ImpossibleSecondsRelativeConstraints(NodePtr),
    ImpossibleSecondsAbsoluteConstraints(NodePtr),
    ImpossibleHeightRelativeConstraints(NodePtr),
    ImpossibleHeightAbsoluteConstraints(NodePtr),
    AssertMyBirthSecondsFailed(NodePtr),
    AssertMyBirthHeightFailed(NodePtr),
    AssertEphemeralFailed(NodePtr),
    EphemeralRelativeCondition(NodePtr),
    InvalidSoftforkCondition(NodePtr),
    InvalidSoftforkCost(NodePtr),
    TooManyAnnouncements(NodePtr),
    InvalidMessageMode(Option<NodePtr>),
    InvalidCoinId(NodePtr),
    MessageNotSentOrReceived,
    ComplexGeneratorReceived,
}

impl From<EvalErr> for ErrorCode {
    fn from(v: EvalErr) -> Self {
        match v {
            EvalErr::CostExceeded => ErrorCode::CostExceeded(Some(v.node_ptr())),
            _ => ErrorCode::GeneratorRuntimeError(Some(v.node_ptr())),
        }
    }
}

impl From<std::io::Error> for ErrorCode {
    fn from(_: std::io::Error) -> Self {
        ErrorCode::GeneratorRuntimeError(None)
    }
}

#[cfg(feature = "py-bindings")]
impl From<ErrorCode> for PyErr {
    fn from(err: ErrorCode) -> PyErr {
        pyo3::exceptions::PyValueError::new_err(("ValidationError", u32::from(err)))
    }
}

// helper functions that fail with ErrorCode
pub fn first(a: &Allocator, n: NodePtr) -> Result<NodePtr, ErrorCode> {
    match a.sexp(n) {
        SExp::Pair(left, _) => Ok(left),
        SExp::Atom => Err(ErrorCode::InvalidCondition(n)),
    }
}

// from chia-blockchain/chia/util/errors.py
impl From<ErrorCode> for u32 {
    fn from(err: ErrorCode) -> u32 {
        match err {
            ErrorCode::Unknown => 1,
            ErrorCode::InvalidBlockSolution(..) => 2,
            ErrorCode::InvalidCoinSolution(..) => 3,
            ErrorCode::DuplicateOutput(..) => 4,
            ErrorCode::DoubleSpend(..) => 5,
            ErrorCode::UnknownUnspent(..) => 6,
            ErrorCode::BadAggregateSignature => 7,
            ErrorCode::WrongPuzzleHash(..) => 8,
            ErrorCode::BadFarmerCoinAmount(..) => 9,
            ErrorCode::InvalidCondition(..)
            | ErrorCode::InvalidConditionOpcode(..)
            | ErrorCode::InvalidParentId(..)
            | ErrorCode::InvalidPuzzleHash(..)
            | ErrorCode::InvalidPublicKey(..)
            | ErrorCode::InvalidMessage(..)
            | ErrorCode::InvalidCoinAmount(..)
            | ErrorCode::InvalidCoinAnnouncement(..)
            | ErrorCode::InvalidPuzzleAnnouncement(..) => 10,
            ErrorCode::AssertMyCoinIdFailed(..) => 11,
            ErrorCode::AssertPuzzleAnnouncementFailed(..)
            | ErrorCode::AssertCoinAnnouncementFailed(..) => {
                12
            }
            ErrorCode::AssertHeightRelativeFailed(..) => 13,
            ErrorCode::AssertHeightAbsoluteFailed(..) => 14,
            ErrorCode::AssertSecondsAbsoluteFailed(..) => 15,
            ErrorCode::CoinAmountExceedsMaximum(..) => 16,
            ErrorCode::SexpError(..) => 17,
            ErrorCode::InvalidFeeLowFee(..) => 18,
            ErrorCode::MempoolConflict(..) => 19,
            ErrorCode::MintingCoin(..) => 20,
            ErrorCode::ExtendsUnknownBlock(..) => 21,
            ErrorCode::CoinbaseNotYetSpendable(..) => 22,
            ErrorCode::CostExceeded(_) => 23,
            ErrorCode::BadAdditionRoot(..) => 24,
            ErrorCode::BadRemovalRoot(..) => 25,
            ErrorCode::InvalidPospaceHash(..) => 26,
            ErrorCode::InvalidCoinbaseSignature(..) => 27,
            ErrorCode::InvalidPlotSignature(..) => 28,
            ErrorCode::TimestampTooFarInPast(..) => 29,
            ErrorCode::TimestampTooFarInFuture(..) => 30,
            ErrorCode::InvalidTransactionsFilterHash(..) => 31,
            ErrorCode::InvalidPospaceChallenge(..) => 32,
            ErrorCode::InvalidPospace(..) => 33,
            ErrorCode::InvalidHeight(..) => 34,
            ErrorCode::InvalidCoinbaseAmount(..) => 35,
            ErrorCode::InvalidMerkleRoot(..) => 36,
            ErrorCode::InvalidBlockFeeAmount(..) => 37,
            ErrorCode::InvalidWeight(..) => 38,
            ErrorCode::InvalidTotalIters(..) => 39,
            ErrorCode::BlockIsNotFinished(..) => 40,
            ErrorCode::InvalidNumIterations(..) => 41,
            ErrorCode::InvalidPot(..) => 42,
            ErrorCode::InvalidPotChallenge(..) => 43,
            ErrorCode::InvalidTransactionsGeneratorHash(..) => 44,
            ErrorCode::InvalidPoolTarget(..) => 45,
            ErrorCode::InvalidCoinbaseParent(..) => 46,
            ErrorCode::InvalidFeesCoinParent(..) => 47,
            ErrorCode::ReserveFeeConditionFailed(..) => 48,
            ErrorCode::NotBlockButHasData(..) => 49,
            ErrorCode::IsTransactionBlockButNoData(..) => 50,
            ErrorCode::InvalidPrevBlockHash(..) => 51,
            ErrorCode::InvalidTransactionsInfoHash(..) => 52,
            ErrorCode::InvalidFoliageBlockHash(..) => 53,
            ErrorCode::InvalidRewardCoins(..) => 54,
            ErrorCode::InvalidBlockCost(..) => 55,
            ErrorCode::NoEndOfSlotInfo(..) => 56,
            ErrorCode::InvalidPrevChallengeSlotHash(..) => 57,
            ErrorCode::InvalidSubEpochSummaryHash(..) => 58,
            ErrorCode::NoSubEpochSummaryHash(..) => 59,
            ErrorCode::ShouldNotMakeChallengeBlock(..) => 60,
            ErrorCode::ShouldMakeChallengeBlock(..) => 61,
            ErrorCode::InvalidChallengeChainData(..) => 62,
            ErrorCode::InvalidCcEosVdf(..) => 65,
            ErrorCode::InvalidRcEosVdf(..) => 66,
            ErrorCode::InvalidChallengeSlotHashRc(..) => 67,
            ErrorCode::InvalidPriorPointRc(..) => 68,
            ErrorCode::InvalidDeficit(..) => 69,
            ErrorCode::InvalidSubEpochSummary(..) => 70,
            ErrorCode::InvalidPrevSubEpochSummaryHash(..) => 71,
            ErrorCode::InvalidRewardChainHash(..) => 72,
            ErrorCode::InvalidSubEpochOverflow(..) => 73,
            ErrorCode::InvalidNewDifficulty(..) => 74,
            ErrorCode::InvalidNewSubSlotIters(..) => 75,
            ErrorCode::InvalidCcSpVdf(..) => 76,
            ErrorCode::InvalidRcSpVdf(..) => 77,
            ErrorCode::InvalidCcSignature(..) => 78,
            ErrorCode::InvalidRcSignature(..) => 79,
            ErrorCode::CannotMakeCcBlock(..) => 80,
            ErrorCode::InvalidRcSpPrevIp(..) => 81,
            ErrorCode::InvalidRcIpPrevIp(..) => 82,
            ErrorCode::InvalidIsTransactionBlock(..) => 83,
            ErrorCode::InvalidUrsbHash(..) => 84,
            ErrorCode::OldPoolTarget(..) => 85,
            ErrorCode::InvalidPoolSignature(..) => 86,
            ErrorCode::InvalidFoliageBlockPresence(..) => 87,
            ErrorCode::InvalidCcIpVdf(..) => 88,
            ErrorCode::InvalidRcIpVdf(..) => 89,
            ErrorCode::IpShouldBeNone(..) => 90,
            ErrorCode::InvalidRewardBlockHash(..) => 91,
            ErrorCode::InvalidMadeNonOverflowInfusions(..) => 92,
            ErrorCode::NoOverflowsInFirstSubSlotNewEpoch(..) => 93,
            ErrorCode::MempoolNotInitialized(..) => 94,
            ErrorCode::ShouldNotHaveIcc(..) => 95,
            ErrorCode::ShouldHaveIcc(..) => 96,
            ErrorCode::InvalidIccVdf(..) => 97,
            ErrorCode::InvalidIccHashCc(..) => 98,
            ErrorCode::InvalidIccHashRc(..) => 99,
            ErrorCode::InvalidIccEosVdf(..) => 100,
            ErrorCode::InvalidSpIndex(..) => 101,
            ErrorCode::TooManyBlocks(..) => 102,
            ErrorCode::InvalidCcChallenge(..) => 103,
            ErrorCode::InvalidPrefarm(..) => 104,
            ErrorCode::AssertSecondsRelativeFailed(..) => 105,
            ErrorCode::BadCoinbaseSignature(..) => 106,
            // ErrorCode::InitialTransactionFreeze => 107 (removed in `chia-blockchain`` as well)
            ErrorCode::NoTransactionsWhileSyncing(..) => 108,
            ErrorCode::AlreadyIncludingTransaction(..) => 109,
            ErrorCode::IncompatibleNetworkId(..) => 110,
            ErrorCode::PreSoftForkMaxGeneratorSize(..) => 111,
            ErrorCode::InvalidRequiredIters(..) => 112,
            ErrorCode::TooManyGeneratorRefs => 113,
            ErrorCode::AssertMyParentIdFailed(..) => 114,
            ErrorCode::AssertMyPuzzleHashFailed(..) => 115,
            ErrorCode::AssertMyAmountFailed(..) => 116,
            ErrorCode::GeneratorRuntimeError(_) => 117,
            ErrorCode::InvalidCostResult(..) => 118,
            ErrorCode::InvalidTransactionsGeneratorRefsRoot(..) => 119,
            ErrorCode::FutureGeneratorRefs(..) => 120,
            ErrorCode::GeneratorRefHasNoGenerator(..) => 121,
            ErrorCode::DoubleSpendInFork(..) => 122,
            ErrorCode::InvalidFeeTooCloseToZero(..) => 123,
            ErrorCode::CoinAmountNegative(..) => 124,
            ErrorCode::InternalProtocolError(..) => 125,
            ErrorCode::InvalidSpendBundle(..) => 126,
            ErrorCode::FailedGettingGeneratorMultiprocessing(..) => 127,
            ErrorCode::AssertBeforeSecondsAbsoluteFailed(..) => 128,
            ErrorCode::AssertBeforeSecondsRelativeFailed(..) => 129,
            ErrorCode::AssertBeforeHeightAbsoluteFailed(..) => 130,
            ErrorCode::AssertBeforeHeightRelativeFailed(..) => 131,
            ErrorCode::AssertConcurrentSpendFailed(..) => 132,
            ErrorCode::AssertConcurrentPuzzleFailed(..) => 133,
            ErrorCode::ImpossibleSecondsRelativeConstraints(..) => 134,
            ErrorCode::ImpossibleSecondsAbsoluteConstraints(..) => 135,
            ErrorCode::ImpossibleHeightRelativeConstraints(..) => 136,
            ErrorCode::ImpossibleHeightAbsoluteConstraints(..) => 137,
            ErrorCode::AssertMyBirthSecondsFailed(..) => 138,
            ErrorCode::AssertMyBirthHeightFailed(..) => 139,
            ErrorCode::AssertEphemeralFailed(..) => 140,
            ErrorCode::EphemeralRelativeCondition(..) => 141,
            ErrorCode::InvalidSoftforkCondition(..) => 142,
            ErrorCode::InvalidSoftforkCost(..) => 143,
            ErrorCode::TooManyAnnouncements(..) => 144,
            ErrorCode::InvalidMessageMode(_) => 145,
            ErrorCode::InvalidCoinId(..) => 146,
            ErrorCode::MessageNotSentOrReceived => 147,
            ErrorCode::ComplexGeneratorReceived => 148,
        }
    }
}

pub fn rest(a: &Allocator, n: NodePtr) -> Result<NodePtr, ErrorCode> {
    match a.sexp(n) {
        SExp::Pair(_, right) => Ok(right),
        SExp::Atom => Err(ErrorCode::InvalidCondition(n)),
    }
}

pub fn next(a: &Allocator, n: NodePtr) -> Result<Option<(NodePtr, NodePtr)>, ErrorCode> {
    match a.sexp(n) {
        SExp::Pair(left, right) => Ok(Some((left, right))),
        SExp::Atom => {
            // this is expected to be a valid list terminator
            if a.atom_len(n) == 0 {
                Ok(None)
            } else {
                Err(ErrorCode::InvalidCondition(n))
            }
        }
    }
}

pub fn atom(
    a: &Allocator,
    n: NodePtr,
    code: impl Fn(NodePtr) -> ErrorCode + Copy,
) -> Result<Atom<'_>, ErrorCode> {
    match a.sexp(n) {
        SExp::Atom => Ok(a.atom(n)),
        SExp::Pair(..) => Err(code(n)),
    }
}

pub fn check_nil(a: &Allocator, n: NodePtr) -> Result<(), ErrorCode> {
    if atom(a, n, ErrorCode::InvalidCondition)?.as_ref().is_empty() {
        Ok(())
    } else {
        Err(ErrorCode::InvalidCondition(n))
    }
}
