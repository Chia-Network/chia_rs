use chia_streamable_macro::streamable;

use crate::Bytes32;
use crate::ProofOfSpace;
use crate::VDFInfo;
use chia_bls::G2Element;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[streamable]
pub struct RewardChainBlockUnfinished {
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    reward_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
}

#[streamable]
pub struct RewardChainBlock {
    weight: u128,
    height: u32,
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    challenge_chain_ip_vdf: VDFInfo,
    reward_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
    reward_chain_ip_vdf: VDFInfo,
    infused_challenge_chain_ip_vdf: Option<VDFInfo>, // Iff deficit < 16
    is_transaction_block: bool,
}

#[cfg_attr(feature = "py-bindings", pymethods)]
impl RewardChainBlock {
    pub fn get_unfinished(&self) -> RewardChainBlockUnfinished {
        RewardChainBlockUnfinished {
            total_iters: self.total_iters,
            signage_point_index: self.signage_point_index,
            pos_ss_cc_challenge_hash: self.pos_ss_cc_challenge_hash,
            proof_of_space: self.proof_of_space.clone(),
            challenge_chain_sp_vdf: self.challenge_chain_sp_vdf.clone(),
            challenge_chain_sp_signature: self.challenge_chain_sp_signature.clone(),
            reward_chain_sp_vdf: self.reward_chain_sp_vdf.clone(),
            reward_chain_sp_signature: self.reward_chain_sp_signature.clone(),
        }
    }
}
