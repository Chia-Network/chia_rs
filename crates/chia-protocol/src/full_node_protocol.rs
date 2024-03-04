use chia_streamable_macro::Streamable;

use crate::message_struct;
use crate::ChiaProtocolMessage;
use crate::EndOfSubSlotBundle;
use crate::FullBlock;
use crate::SpendBundle;
use crate::TimestampedPeerInfo;
use crate::UnfinishedBlock;
use crate::VDFInfo;
use crate::VDFProof;
use crate::WeightProof;
use crate::{Bytes, Bytes32};

message_struct!(NewPeak {
    header_hash: Bytes32,
    height: u32,
    weight: u128,
    fork_point_with_previous_peak: u32,
    unfinished_reward_block_hash: Bytes32,
});

message_struct!(NewTransaction {
    transaction_id: Bytes32,
    cost: u64,
    fees: u64,
});

message_struct!(RequestTransaction {
    transaction_id: Bytes32,
});

message_struct!(RespondTransaction {
    transaction: SpendBundle,
});

message_struct!(RequestProofOfWeight {
    total_number_of_blocks: u32,
    tip: Bytes32,
});

message_struct!(RespondProofOfWeight {
    wp: WeightProof,
    tip: Bytes32,
});

message_struct!(RequestBlock {
    height: u32,
    include_transaction_block: bool,
});

message_struct!(RejectBlock { height: u32 });

message_struct!(RequestBlocks {
    start_height: u32,
    end_height: u32,
    include_transaction_block: bool,
});

message_struct!(RespondBlocks {
    start_height: u32,
    end_height: u32,
    blocks: Vec<FullBlock>,
});

message_struct!(RejectBlocks {
    start_height: u32,
    end_height: u32,
});

message_struct!(RespondBlock { block: FullBlock });

message_struct!(NewUnfinishedBlock {
    unfinished_reward_hash: Bytes32,
});

message_struct!(RequestUnfinishedBlock {
    unfinished_reward_hash: Bytes32,
});

message_struct!(RespondUnfinishedBlock {
    unfinished_block: UnfinishedBlock,
});

message_struct!(NewSignagePointOrEndOfSubSlot {
    prev_challenge_hash: Option<Bytes32>,
    challenge_hash: Bytes32,
    index_from_challenge: u8,
    last_rc_infusion: Bytes32,
});

message_struct!(RequestSignagePointOrEndOfSubSlot {
    challenge_hash: Bytes32,
    index_from_challenge: u8,
    last_rc_infusion: Bytes32,
});

message_struct!(RespondSignagePoint {
    index_from_challenge: u8,
    challenge_chain_vdf: VDFInfo,
    challenge_chain_proof: VDFProof,
    reward_chain_vdf: VDFInfo,
    reward_chain_proof: VDFProof,
});

message_struct!(RespondEndOfSubSlot {
    end_of_slot_bundle: EndOfSubSlotBundle,
});

message_struct!(RequestMempoolTransactions { filter: Bytes });

message_struct!(NewCompactVDF {
    height: u32,
    header_hash: Bytes32,
    field_vdf: u8,
    vdf_info: VDFInfo,
});

message_struct!(RequestCompactVDF {
    height: u32,
    header_hash: Bytes32,
    field_vdf: u8,
    vdf_info: VDFInfo,
});

message_struct!(RespondCompactVDF {
    height: u32,
    header_hash: Bytes32,
    field_vdf: u8,
    vdf_info: VDFInfo,
    vdf_proof: VDFProof,
});

message_struct!(RequestPeers {});

message_struct!(RespondPeers {
    peer_list: Vec<TimestampedPeerInfo>,
});
