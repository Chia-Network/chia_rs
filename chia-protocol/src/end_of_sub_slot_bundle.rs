use chia_streamable_macro::Streamable;

use crate::streamable_struct;
use crate::ChallengeChainSubSlot;
use crate::InfusedChallengeChainSubSlot;
use crate::RewardChainSubSlot;
use crate::SubSlotProofs;

streamable_struct! (EndOfSubSlotBundle {
    challenge_chain: ChallengeChainSubSlot,
    infused_challenge_chain: Option<InfusedChallengeChainSubSlot>,
    reward_chain: RewardChainSubSlot,
    proofs: SubSlotProofs,
});
