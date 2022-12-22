use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::streamable_struct;
use crate::ChallengeChainSubSlot;
use crate::InfusedChallengeChainSubSlot;
use crate::RewardChainSubSlot;
use crate::Streamable;
use crate::SubSlotProofs;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct! (EndOfSubSlotBundle {
    challenge_chain: ChallengeChainSubSlot,
    infused_challenge_chain: Option<InfusedChallengeChainSubSlot>,
    reward_chain: RewardChainSubSlot,
    proofs: SubSlotProofs,
});
