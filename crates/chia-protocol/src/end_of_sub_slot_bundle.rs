use chia_streamable_macro::streamable;

use crate::ChallengeChainSubSlot;
use crate::InfusedChallengeChainSubSlot;
use crate::RewardChainSubSlot;
use crate::SubSlotProofs;

#[streamable]
pub struct EndOfSubSlotBundle {
    challenge_chain: ChallengeChainSubSlot,
    infused_challenge_chain: Option<InfusedChallengeChainSubSlot>,
    reward_chain: RewardChainSubSlot,
    proofs: SubSlotProofs,
}
