use chia_sha2::Sha256;
use chia_streamable_macro::streamable;
use chia_traits::{Result, Streamable};
use std::io::Cursor;

use crate::Bytes32;
use crate::EndOfSubSlotBundle;
use crate::HeaderBlock;
use crate::ProofOfSpace;
use crate::RewardChainBlock;
use crate::utils::{parse, stream, update_digest};
use crate::{VDFInfo, VDFProof};

#[streamable(no_streamable)]
pub struct SubEpochData {
    reward_chain_hash: Bytes32,
    num_blocks_overflow: u8,
    new_sub_slot_iters: Option<u64>,
    new_difficulty: Option<u64>,
    challenge_merkle_root: Option<Bytes32>,
}
impl Streamable for SubEpochData {
    fn update_digest(&self, digest: &mut Sha256) {
        self.reward_chain_hash.update_digest(digest);
        self.num_blocks_overflow.update_digest(digest);
        self.new_sub_slot_iters.update_digest(digest);
        update_digest(
            self.new_difficulty.as_ref(),
            self.challenge_merkle_root.as_ref(),
            digest,
        );
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.reward_chain_hash.stream(out)?;
        self.num_blocks_overflow.stream(out)?;
        self.new_sub_slot_iters.stream(out)?;
        stream(
            self.new_difficulty.as_ref(),
            self.challenge_merkle_root.as_ref(),
            out,
        )
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let rch = <Bytes32 as Streamable>::parse::<TRUSTED>(input)?;
        let nbo = <u8 as Streamable>::parse::<TRUSTED>(input)?;
        let nssi = <Option<u64> as Streamable>::parse::<TRUSTED>(input)?;
        let (nd, challenge_merkle_root) = parse::<TRUSTED, u64, Bytes32>(input)?;
        Ok(Self {
            reward_chain_hash: rch,
            num_blocks_overflow: nbo,
            new_sub_slot_iters: nssi,
            new_difficulty: nd,
            challenge_merkle_root,
        })
    }
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
