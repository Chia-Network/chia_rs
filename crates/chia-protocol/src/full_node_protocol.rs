use chia_streamable_macro::streamable;

use crate::EndOfSubSlotBundle;
use crate::FullBlock;
use crate::SpendBundle;
use crate::TimestampedPeerInfo;
use crate::UnfinishedBlock;
use crate::VDFInfo;
use crate::VDFProof;
use crate::WeightProof;
use crate::{Bytes, Bytes32};

#[streamable(message)]
pub struct NewPeak {
    header_hash: Bytes32,
    height: u32,
    weight: u128,
    fork_point_with_previous_peak: u32,
    unfinished_reward_block_hash: Bytes32,
}

#[streamable(message)]
pub struct NewTransaction {
    transaction_id: Bytes32,
    cost: u64,
    fees: u64,
}

#[streamable(message)]
pub struct RequestTransaction {
    transaction_id: Bytes32,
}

#[streamable(message)]
pub struct RespondTransaction {
    transaction: SpendBundle,
}

#[streamable(message)]
pub struct RequestProofOfWeight {
    total_number_of_blocks: u32,
    tip: Bytes32,
}

#[streamable(message)]
pub struct RespondProofOfWeight {
    wp: WeightProof,
    tip: Bytes32,
}

#[streamable(message)]
pub struct RequestBlock {
    height: u32,
    include_transaction_block: bool,
}

#[streamable(message)]
pub struct RejectBlock {
    height: u32,
}

#[streamable(message)]
pub struct RequestBlocks {
    start_height: u32,
    end_height: u32,
    include_transaction_block: bool,
}

#[streamable(message)]
pub struct RespondBlocks {
    start_height: u32,
    end_height: u32,
    blocks: Vec<FullBlock>,
}

#[streamable(message)]
pub struct RejectBlocks {
    start_height: u32,
    end_height: u32,
}

#[streamable(message)]
pub struct RespondBlock {
    block: FullBlock,
}

#[streamable(message)]
pub struct NewUnfinishedBlock {
    unfinished_reward_hash: Bytes32,
}

#[streamable(message)]
pub struct RequestUnfinishedBlock {
    unfinished_reward_hash: Bytes32,
}

#[streamable(message)]
pub struct RespondUnfinishedBlock {
    unfinished_block: UnfinishedBlock,
}

#[streamable(message)]
pub struct NewSignagePointOrEndOfSubSlot {
    prev_challenge_hash: Option<Bytes32>,
    challenge_hash: Bytes32,
    index_from_challenge: u8,
    last_rc_infusion: Bytes32,
}

#[streamable(message)]
pub struct RequestSignagePointOrEndOfSubSlot {
    challenge_hash: Bytes32,
    index_from_challenge: u8,
    last_rc_infusion: Bytes32,
}

#[streamable(message)]
pub struct RespondSignagePoint {
    index_from_challenge: u8,
    challenge_chain_vdf: VDFInfo,
    challenge_chain_proof: VDFProof,
    reward_chain_vdf: VDFInfo,
    reward_chain_proof: VDFProof,
}

#[streamable(message)]
pub struct RespondEndOfSubSlot {
    end_of_slot_bundle: EndOfSubSlotBundle,
}

#[streamable(message)]
pub struct RequestMempoolTransactions {
    filter: Bytes,
}

#[streamable(message)]
pub struct NewCompactVDF {
    height: u32,
    header_hash: Bytes32,
    field_vdf: u8,
    vdf_info: VDFInfo,
}

#[streamable(message)]
pub struct RequestCompactVDF {
    height: u32,
    header_hash: Bytes32,
    field_vdf: u8,
    vdf_info: VDFInfo,
}

#[streamable(message)]
pub struct RespondCompactVDF {
    height: u32,
    header_hash: Bytes32,
    field_vdf: u8,
    vdf_info: VDFInfo,
    vdf_proof: VDFProof,
}

#[streamable(message)]
pub struct RequestPeers {}

#[streamable(message)]
pub struct RespondPeers {
    peer_list: Vec<TimestampedPeerInfo>,
}

#[streamable(message)]
pub struct NewUnfinishedBlock2 {
    unfinished_reward_hash: Bytes32,
    foliage_hash: Option<Bytes32>,
}

#[streamable(message)]
pub struct RequestUnfinishedBlock2 {
    unfinished_reward_hash: Bytes32,
    foliage_hash: Option<Bytes32>,
}
