use chia_streamable_macro::streamable;

use crate::Bytes32;
use crate::ProofOfSpace;
use crate::VDFInfo;
use chia_bls::G2Element;
use chia_sha2::Sha256;
use chia_traits::{Result, Streamable};
use std::io::Cursor;

use crate::utils::{parse, stream, update_digest};

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

#[streamable(no_streamable)]
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
    header_mmr_root: Option<Bytes32>,
    is_transaction_block: bool,
}

impl Streamable for RewardChainBlock {
    fn update_digest(&self, digest: &mut Sha256) {
        self.weight.update_digest(digest);
        self.height.update_digest(digest);
        self.total_iters.update_digest(digest);
        self.signage_point_index.update_digest(digest);
        self.pos_ss_cc_challenge_hash.update_digest(digest);
        self.proof_of_space.update_digest(digest);
        self.challenge_chain_sp_vdf.update_digest(digest);
        self.challenge_chain_sp_signature.update_digest(digest);
        self.challenge_chain_ip_vdf.update_digest(digest);
        self.reward_chain_sp_vdf.update_digest(digest);
        self.reward_chain_sp_signature.update_digest(digest);
        self.reward_chain_ip_vdf.update_digest(digest);
        update_digest(
            self.infused_challenge_chain_ip_vdf.as_ref(),
            self.header_mmr_root.as_ref(),
            digest,
        );
        self.is_transaction_block.update_digest(digest);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.weight.stream(out)?;
        self.height.stream(out)?;
        self.total_iters.stream(out)?;
        self.signage_point_index.stream(out)?;
        self.pos_ss_cc_challenge_hash.stream(out)?;
        self.proof_of_space.stream(out)?;
        self.challenge_chain_sp_vdf.stream(out)?;
        self.challenge_chain_sp_signature.stream(out)?;
        self.challenge_chain_ip_vdf.stream(out)?;
        self.reward_chain_sp_vdf.stream(out)?;
        self.reward_chain_sp_signature.stream(out)?;
        self.reward_chain_ip_vdf.stream(out)?;
        stream(
            self.infused_challenge_chain_ip_vdf.as_ref(),
            self.header_mmr_root.as_ref(),
            out,
        )?;
        self.is_transaction_block.stream(out)?;
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let weight = <u128 as Streamable>::parse::<TRUSTED>(input)?;
        let height = <u32 as Streamable>::parse::<TRUSTED>(input)?;
        let total_iters = <u128 as Streamable>::parse::<TRUSTED>(input)?;
        let signage_point_index = <u8 as Streamable>::parse::<TRUSTED>(input)?;
        let pos_ss_cc_challenge_hash = <Bytes32 as Streamable>::parse::<TRUSTED>(input)?;
        let proof_of_space = <ProofOfSpace as Streamable>::parse::<TRUSTED>(input)?;
        let challenge_chain_sp_vdf = <Option<VDFInfo> as Streamable>::parse::<TRUSTED>(input)?;
        let challenge_chain_sp_signature = <G2Element as Streamable>::parse::<TRUSTED>(input)?;
        let challenge_chain_ip_vdf = <VDFInfo as Streamable>::parse::<TRUSTED>(input)?;
        let reward_chain_sp_vdf = <Option<VDFInfo> as Streamable>::parse::<TRUSTED>(input)?;
        let reward_chain_sp_signature = <G2Element as Streamable>::parse::<TRUSTED>(input)?;
        let reward_chain_ip_vdf = <VDFInfo as Streamable>::parse::<TRUSTED>(input)?;
        let (infused_challenge_chain_ip_vdf, header_mmr_root) =
            parse::<TRUSTED, VDFInfo, Bytes32>(input)?;
        let is_transaction_block = <bool as Streamable>::parse::<TRUSTED>(input)?;

        Ok(Self {
            weight,
            height,
            total_iters,
            signage_point_index,
            pos_ss_cc_challenge_hash,
            proof_of_space,
            challenge_chain_sp_vdf,
            challenge_chain_sp_signature,
            challenge_chain_ip_vdf,
            reward_chain_sp_vdf,
            reward_chain_sp_signature,
            reward_chain_ip_vdf,
            infused_challenge_chain_ip_vdf,
            header_mmr_root,
            is_transaction_block,
        })
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
}
