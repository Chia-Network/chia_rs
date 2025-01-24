use chia_streamable_macro::{streamable, Streamable};

use crate::Coin;
use crate::CoinState;
use crate::FeeEstimateGroup;
use crate::HeaderBlock;
use crate::Program;
use crate::SpendBundle;
use crate::{Bytes, Bytes32};

#[streamable(message)]
pub struct RequestPuzzleSolution {
    coin_name: Bytes32,
    height: u32,
}

#[streamable]
pub struct PuzzleSolutionResponse {
    coin_name: Bytes32,
    height: u32,
    puzzle: Program,
    solution: Program,
}

#[streamable(message)]
pub struct RespondPuzzleSolution {
    response: PuzzleSolutionResponse,
}

#[streamable(message)]
pub struct RejectPuzzleSolution {
    coin_name: Bytes32,
    height: u32,
}

#[streamable(message)]
pub struct SendTransaction {
    transaction: SpendBundle,
}

#[streamable(message)]
pub struct TransactionAck {
    txid: Bytes32,
    status: u8, // MempoolInclusionStatus
    error: Option<String>,
}

#[streamable(message)]
pub struct NewPeakWallet {
    header_hash: Bytes32,
    height: u32,
    weight: u128,
    fork_point_with_previous_peak: u32,
}

#[streamable(message)]
pub struct RequestBlockHeader {
    height: u32,
}

#[streamable(message)]
pub struct RespondBlockHeader {
    header_block: HeaderBlock,
}

#[streamable(message)]
pub struct RejectHeaderRequest {
    height: u32,
}

#[streamable(message)]
pub struct RequestRemovals {
    height: u32,
    header_hash: Bytes32,
    coin_names: Option<Vec<Bytes32>>,
}

#[streamable(message)]
pub struct RespondRemovals {
    height: u32,
    header_hash: Bytes32,
    coins: Vec<(Bytes32, Option<Coin>)>,
    proofs: Option<Vec<(Bytes32, Bytes)>>,
}

#[streamable(message)]
pub struct RejectRemovalsRequest {
    height: u32,
    header_hash: Bytes32,
}

#[streamable(message)]
pub struct RequestAdditions {
    height: u32,
    header_hash: Option<Bytes32>,
    puzzle_hashes: Option<Vec<Bytes32>>,
}

#[streamable(message)]
pub struct RespondAdditions {
    height: u32,
    header_hash: Bytes32,
    coins: Vec<(Bytes32, Vec<Coin>)>,
    proofs: Option<Vec<(Bytes32, Bytes, Option<Bytes>)>>,
}

#[streamable(message)]
pub struct RejectAdditionsRequest {
    height: u32,
    header_hash: Bytes32,
}

#[streamable(message)]
pub struct RespondBlockHeaders {
    start_height: u32,
    end_height: u32,
    header_blocks: Vec<HeaderBlock>,
}

#[streamable(message)]
pub struct RejectBlockHeaders {
    start_height: u32,
    end_height: u32,
}

#[streamable(message)]
pub struct RequestBlockHeaders {
    start_height: u32,
    end_height: u32,
    return_filter: bool,
}

#[streamable(message)]
pub struct RequestHeaderBlocks {
    start_height: u32,
    end_height: u32,
}

#[streamable(message)]
pub struct RejectHeaderBlocks {
    start_height: u32,
    end_height: u32,
}

#[streamable(message)]
pub struct RespondHeaderBlocks {
    start_height: u32,
    end_height: u32,
    header_blocks: Vec<HeaderBlock>,
}

#[streamable(message)]
pub struct RegisterForPhUpdates {
    puzzle_hashes: Vec<Bytes32>,
    min_height: u32,
}

#[streamable(message)]
pub struct RespondToPhUpdates {
    puzzle_hashes: Vec<Bytes32>,
    min_height: u32,
    coin_states: Vec<CoinState>,
}

#[streamable(message)]
pub struct RegisterForCoinUpdates {
    coin_ids: Vec<Bytes32>,
    min_height: u32,
}

#[streamable(message)]
pub struct RespondToCoinUpdates {
    coin_ids: Vec<Bytes32>,
    min_height: u32,
    coin_states: Vec<CoinState>,
}

#[streamable(message)]
pub struct CoinStateUpdate {
    height: u32,
    fork_height: u32,
    peak_hash: Bytes32,
    items: Vec<CoinState>,
}

#[streamable(message)]
pub struct RequestChildren {
    coin_name: Bytes32,
}

#[streamable(message)]
pub struct RespondChildren {
    coin_states: Vec<CoinState>,
}

#[streamable(message)]
pub struct RequestSesInfo {
    start_height: u32,
    end_height: u32,
}

#[streamable(message)]
pub struct RespondSesInfo {
    reward_chain_hash: Vec<Bytes32>,
    heights: Vec<Vec<u32>>,
}

#[streamable(message)]
pub struct RequestFeeEstimates {
    time_targets: Vec<u64>,
}

#[streamable(message)]
pub struct RespondFeeEstimates {
    estimates: FeeEstimateGroup,
}

#[streamable(message)]
pub struct RequestRemovePuzzleSubscriptions {
    puzzle_hashes: Option<Vec<Bytes32>>,
}

#[streamable(message)]
pub struct RespondRemovePuzzleSubscriptions {
    puzzle_hashes: Vec<Bytes32>,
}

#[streamable(message)]
pub struct RequestRemoveCoinSubscriptions {
    coin_ids: Option<Vec<Bytes32>>,
}

#[streamable(message)]
pub struct RespondRemoveCoinSubscriptions {
    coin_ids: Vec<Bytes32>,
}

#[streamable]
pub struct CoinStateFilters {
    include_spent: bool,
    include_unspent: bool,
    include_hinted: bool,
    min_amount: u64,
}

#[streamable(message)]
pub struct RequestPuzzleState {
    puzzle_hashes: Vec<Bytes32>,
    previous_height: Option<u32>,
    header_hash: Bytes32,
    filters: CoinStateFilters,
    subscribe_when_finished: bool,
}

#[streamable(message)]
pub struct RespondPuzzleState {
    puzzle_hashes: Vec<Bytes32>,
    height: u32,
    header_hash: Bytes32,
    is_finished: bool,
    coin_states: Vec<CoinState>,
}

#[streamable(message)]
pub struct RejectPuzzleState {
    reason: RejectStateReason,
}

#[streamable(message)]
pub struct RequestCoinState {
    coin_ids: Vec<Bytes32>,
    previous_height: Option<u32>,
    header_hash: Bytes32,
    subscribe: bool,
}

#[streamable(message)]
pub struct RespondCoinState {
    coin_ids: Vec<Bytes32>,
    coin_states: Vec<CoinState>,
}

#[streamable(message)]
pub struct RejectCoinState {
    reason: RejectStateReason,
}

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};

#[repr(u8)]
#[cfg_attr(feature = "py-bindings", derive(PyJsonDict, PyStreamable))]
#[derive(Streamable, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum RejectStateReason {
    Reorg = 0,
    ExceededSubscriptionLimit = 1,
}

#[cfg(feature = "py-bindings")]
impl chia_traits::ChiaToPython for RejectStateReason {
    fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
        Ok(pyo3::IntoPyObject::into_pyobject(*self as u8, py)?
            .clone()
            .into_any())
    }
}

#[repr(u8)]
#[cfg_attr(feature = "py-bindings", derive(PyJsonDict, PyStreamable))]
#[derive(Streamable, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum MempoolRemoveReason {
    Conflict = 1,
    BlockInclusion = 2,
    PoolFull = 3,
    Expired = 4,
}

#[cfg(feature = "py-bindings")]
impl chia_traits::ChiaToPython for MempoolRemoveReason {
    fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
        Ok(pyo3::IntoPyObject::into_pyobject(*self as u8, py)?
            .clone()
            .into_any())
    }
}

#[streamable(no_serde)]
pub struct RemovedMempoolItem {
    transaction_id: Bytes32,
    reason: MempoolRemoveReason,
}

#[streamable(message)]
pub struct MempoolItemsAdded {
    transaction_ids: Vec<Bytes32>,
}

#[streamable(message)]
pub struct MempoolItemsRemoved {
    removed_items: Vec<RemovedMempoolItem>,
}

#[streamable(message)]
pub struct RequestCostInfo {}

#[streamable(message)]
pub struct RespondCostInfo {
    max_transaction_cost: u64,
    max_block_cost: u64,
    max_mempool_cost: u64,
    mempool_cost: u64,
    mempool_fee: u64,
    bump_fee_per_cost: u8,
}
