use chia_streamable_macro::streamable;

use crate::Bytes32;
use crate::EndOfSubSlotBundle;
use crate::HeaderBlock;
use crate::ProofOfSpace;
use crate::RewardChainBlock;
use crate::{VDFInfo, VDFProof};

#[streamable]
pub struct SubEpochData {
    reward_chain_hash: Bytes32,
    num_blocks_overflow: u8,
    new_sub_slot_iters: Option<u64>,
    new_difficulty: Option<u64>,
}

// number of challenge blocks
// Average iters for challenge blocks
// |--A-R----R-------R--------R------R----R----------R-----R--R---|       Honest difficulty 1000
//           0.16

//  compute total reward chain blocks
// |----------------------------A---------------------------------|       Attackers chain 1000
//                            0.48
// total number of challenge blocks == total number of reward chain blocks

#[streamable]
pub struct SubSlotData {
    proof_of_space: Option<ProofOfSpace>,
    cc_signage_point: Option<VDFProof>,
    cc_infusion_point: Option<VDFProof>,
    icc_infusion_point: Option<VDFProof>,
    cc_sp_vdf_info: Option<VDFInfo>,
    signage_point_index: Option<u8>,
    cc_slot_end: Option<VDFProof>,
    icc_slot_end: Option<VDFProof>,
    cc_slot_end_info: Option<VDFInfo>,
    icc_slot_end_info: Option<VDFInfo>,
    cc_ip_vdf_info: Option<VDFInfo>,
    icc_ip_vdf_info: Option<VDFInfo>,
    total_iters: Option<u128>,
}

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pymethods)]
impl SubSlotData {
    pub fn is_end_of_slot(&self) -> bool {
        self.cc_slot_end_info.is_some()
    }

    pub fn is_challenge(&self) -> bool {
        self.proof_of_space.is_some()
    }
}

#[streamable]
pub struct SubEpochChallengeSegment {
    sub_epoch_n: u32,
    sub_slots: Vec<SubSlotData>,
    rc_slot_end_info: Option<VDFInfo>,
}

#[streamable]
pub struct SubEpochSegments {
    challenge_segments: Vec<SubEpochChallengeSegment>,
}

// this is used only for serialization to database
#[streamable]
pub struct RecentChainData {
    recent_chain_data: Vec<HeaderBlock>,
}

#[streamable]
pub struct ProofBlockHeader {
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    reward_chain_block: RewardChainBlock,
}

#[streamable]
pub struct WeightProof {
    sub_epochs: Vec<SubEpochData>,
    sub_epoch_segments: Vec<SubEpochChallengeSegment>, // sampled sub epoch
    recent_chain_data: Vec<HeaderBlock>,
}
