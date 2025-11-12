use chia_streamable_macro::streamable;

use crate::Bytes32;
use crate::ProofOfSpace;
use crate::VDFInfo;
use chia_bls::G2Element;
use chia_traits::Streamable;

#[cfg(feature = "py-bindings")]
use pyo3::{exceptions::PyValueError, prelude::*};

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

// Pre-fork structure (before HARD_FORK2_HEIGHT)
#[streamable]
pub struct RewardChainBlockOld {
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

// Post-fork structure (at or after HARD_FORK2_HEIGHT) - this is the default
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
    header_mmr_root: Bytes32, // MMR root of all previous block headers
}

#[cfg_attr(feature = "py-bindings", pymethods)]
impl RewardChainBlockOld {
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

    // Always safe: add zeros for new field
    pub fn to_new(&self) -> RewardChainBlock {
        RewardChainBlock {
            weight: self.weight,
            height: self.height,
            total_iters: self.total_iters,
            signage_point_index: self.signage_point_index,
            pos_ss_cc_challenge_hash: self.pos_ss_cc_challenge_hash,
            proof_of_space: self.proof_of_space.clone(),
            challenge_chain_sp_vdf: self.challenge_chain_sp_vdf.clone(),
            challenge_chain_sp_signature: self.challenge_chain_sp_signature.clone(),
            challenge_chain_ip_vdf: self.challenge_chain_ip_vdf.clone(),
            reward_chain_sp_vdf: self.reward_chain_sp_vdf.clone(),
            reward_chain_sp_signature: self.reward_chain_sp_signature.clone(),
            reward_chain_ip_vdf: self.reward_chain_ip_vdf.clone(),
            infused_challenge_chain_ip_vdf: self.infused_challenge_chain_ip_vdf.clone(),
            is_transaction_block: self.is_transaction_block,
            header_mmr_root: Bytes32::default(), // zeros for pre-fork blocks
        }
    }
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

    // Validated downgrade: only safe if new field is zeros
    #[cfg(feature = "py-bindings")]
    pub fn to_old(&self) -> PyResult<RewardChainBlockOld> {
        if self.header_mmr_root != Bytes32::default() {
            return Err(PyValueError::new_err(
                "Cannot downgrade RewardChainBlock to Old: header_mmr_root is not zeros",
            ));
        }
        Ok(self.to_old_unchecked())
    }

    #[cfg(not(feature = "py-bindings"))]
    pub fn to_old(&self) -> Result<RewardChainBlockOld, String> {
        if self.header_mmr_root != Bytes32::default() {
            return Err(
                "Cannot downgrade RewardChainBlock to Old: header_mmr_root is not zeros"
                    .to_string(),
            );
        }
        Ok(self.to_old_unchecked())
    }

    // Unchecked downgrade: caller guarantees new field is zeros
    pub fn to_old_unchecked(&self) -> RewardChainBlockOld {
        RewardChainBlockOld {
            weight: self.weight,
            height: self.height,
            total_iters: self.total_iters,
            signage_point_index: self.signage_point_index,
            pos_ss_cc_challenge_hash: self.pos_ss_cc_challenge_hash,
            proof_of_space: self.proof_of_space.clone(),
            challenge_chain_sp_vdf: self.challenge_chain_sp_vdf.clone(),
            challenge_chain_sp_signature: self.challenge_chain_sp_signature.clone(),
            challenge_chain_ip_vdf: self.challenge_chain_ip_vdf.clone(),
            reward_chain_sp_vdf: self.reward_chain_sp_vdf.clone(),
            reward_chain_sp_signature: self.reward_chain_sp_signature.clone(),
            reward_chain_ip_vdf: self.reward_chain_ip_vdf.clone(),
            infused_challenge_chain_ip_vdf: self.infused_challenge_chain_ip_vdf.clone(),
            is_transaction_block: self.is_transaction_block,
        }
    }
}
