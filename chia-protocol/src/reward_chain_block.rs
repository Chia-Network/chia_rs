use crate::streamable_struct;
use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::Bytes32;
use crate::G2Element;
use crate::ProofOfSpace;
use crate::Streamable;
use crate::VDFInfo;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct! (RewardChainBlockUnfinished {
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    reward_chain_sp_vdf: Option<VDFInfo>, // Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
});

streamable_struct! (RewardChainBlock {
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
});
