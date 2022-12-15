use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::message_struct;
use crate::streamable_struct;
use crate::ChiaProtocolMessage;
use crate::Coin;
use crate::CoinState;
use crate::FeeEstimateGroup;
use crate::HeaderBlock;
use crate::Program;
use crate::ProtocolMessageTypes;
use crate::SpendBundle;
use crate::Streamable;
use crate::{Bytes, Bytes32};

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

message_struct!(RequestPuzzleSolution {
    coin_name: Bytes32,
    height: u32,
});

streamable_struct!(PuzzleSolutionResponse {
    coin_name: Bytes32,
    height: u32,
    puzzle: Program,
    solution: Program,
});

message_struct!(RespondPuzzleSolution {
    response: PuzzleSolutionResponse,
});

message_struct!(RejectPuzzleSolution {
    coin_name: Bytes32,
    height: u32,
});

message_struct!(SendTransaction {
    ransaction: SpendBundle,
});

message_struct! (TransactionAck {
    txid: Bytes32,
    status: u8, // MempoolInclusionStatus
    error: Option<String>,
});

message_struct!(NewPeakWallet {
    header_hash: Bytes32,
    height: u32,
    weight: u128,
    fork_point_with_previous_peak: u32,
});

message_struct!(RequestBlockHeader { height: u32 });

message_struct!(RespondBlockHeader {
    header_block: HeaderBlock,
});

message_struct!(RejectHeaderRequest { height: u32 });

message_struct! (RequestRemovals {
    height: u32,
    header_hash: Bytes32,
    coin_names: Option<Vec<Bytes32>>,
});

message_struct! (RespondRemovals {
    height: u32,
    header_hash: Bytes32,
    coins: Vec<(Bytes32, Option<Coin>)>,
    proofs: Option<Vec<(Bytes32, Bytes)>>,
});

message_struct!(RejectRemovalsRequest {
    height: u32,
    header_hash: Bytes32,
});

message_struct! (RequestAdditions {
    height: u32,
    header_hash: Option<Bytes32>,
    puzzle_hashes: Option<Vec<Bytes32>>,
});

message_struct! (RespondAdditions {
    height: u32,
    header_hash: Bytes32,
    coins: Vec<(Bytes32, Vec<Coin>)>,
    proofs: Option<Vec<(Bytes32, Bytes, Option<Bytes>)>>,
});

message_struct!(RejectAdditionsRequest {
    height: u32,
    header_hash: Bytes32,
});

message_struct! (RespondBlockHeaders {
    start_height: u32,
    end_height: u32,
    header_blocks: Vec<HeaderBlock>,
});

message_struct!(RejectBlockHeaders {
    start_height: u32,
    end_height: u32,
});

message_struct!(RequestBlockHeaders {
    start_height: u32,
    end_height: u32,
    return_filter: bool,
});

message_struct!(RequestHeaderBlocks {
    start_height: u32,
    end_height: u32,
});

message_struct!(RejectHeaderBlocks {
    start_height: u32,
    end_height: u32,
});

message_struct! (RespondHeaderBlocks {
    start_height: u32,
    end_height: u32,
    header_blocks: Vec<HeaderBlock>,
});

// struct CoinState

message_struct! (RegisterForPhUpdates {
    puzzle_hashes: Vec<Bytes32>,
    min_height: u32,
});

message_struct! (RespondToPhUpdates {
    puzzle_hashes: Vec<Bytes32>,
    min_height: u32,
    coin_states: Vec<CoinState>,
});

message_struct! (RegisterForCoinUpdates {
    coin_ids: Vec<Bytes32>,
    min_height: u32,
});

message_struct! (RespondToCoinUpdates {
    coin_ids: Vec<Bytes32>,
    min_height: u32,
    coin_states: Vec<CoinState>,
});

message_struct! (CoinStateUpdate {
    height: u32,
    fork_height: u32,
    peak_hash: Bytes32,
    items: Vec<CoinState>,
});

message_struct!(RequestChildren { coin_name: Bytes32 });

message_struct! (RespondChildren {
    coin_states: Vec<CoinState>,
});

message_struct!(RequestSesInfo {
    start_height: u32,
    end_height: u32,
});

message_struct! (RespondSesInfo {
    reward_chain_hash: Vec<Bytes32>,
    heights: Vec<Vec<u32>>,
});

message_struct! (RequestFeeEstimates {
    time_targets: Vec<u64>,
});

message_struct!(RespondFeeEstimates {
    estimates: FeeEstimateGroup,
});
