use crate::streamable_struct;
use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::Bytes32;
use crate::G2Element;
use crate::ProofOfSpace;
use crate::Streamable;
use crate::VDFInfo;
use crate::VDFProof;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

// The hash of this is used as the challenge_hash for the ICC VDF
streamable_struct! (ChallengeBlockInfo {
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, // Only present if not the first sp
    challenge_chain_sp_signature: G2Element,
    challenge_chain_ip_vdf: VDFInfo,
});

streamable_struct! (ChallengeChainSubSlot {
    challenge_chain_end_of_slot_vdf: VDFInfo,
    infused_challenge_chain_sub_slot_hash: Option<Bytes32>, // Only at the end of a slot
    subepoch_summary_hash: Option<Bytes32>, // Only once per sub-epoch, and one sub-epoch delayed
    new_sub_slot_iters: Option<u64>,        // Only at the end of epoch, sub-epoch, and slot
    new_difficulty: Option<u64>,            // Only at the end of epoch, sub-epoch, and slot
});

streamable_struct!(InfusedChallengeChainSubSlot {
    infused_challenge_chain_end_of_slot_vdf: VDFInfo,
});

streamable_struct! (RewardChainSubSlot {
    end_of_slot_vdf: VDFInfo,
    challenge_chain_sub_slot_hash: Bytes32,
    infused_challenge_chain_sub_slot_hash: Option<Bytes32>,
    deficit: u8, // 16 or less. usually zero
});

streamable_struct! (SubSlotProofs {
    challenge_chain_slot_proof: VDFProof,
    infused_challenge_chain_slot_proof: Option<VDFProof>,
    reward_chain_slot_proof: VDFProof,
});
