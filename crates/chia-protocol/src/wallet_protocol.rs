use chia_streamable_macro::streamable;

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
