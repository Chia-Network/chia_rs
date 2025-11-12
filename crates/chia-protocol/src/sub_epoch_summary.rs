use chia_streamable_macro::streamable;
use chia_traits::Streamable;

use crate::Bytes32;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

// Old format without challenge_merkle_root, used for pre-fork searialization
#[streamable]
pub struct SubEpochSummaryOld {
    prev_subepoch_summary_hash: Bytes32,
    reward_chain_hash: Bytes32,
    num_blocks_overflow: u8,
    new_difficulty: Option<u64>,
    new_sub_slot_iters: Option<u64>,
}

#[streamable]
pub struct SubEpochSummary {
    prev_subepoch_summary_hash: Bytes32,
    reward_chain_hash: Bytes32,
    num_blocks_overflow: u8,
    new_difficulty: Option<u64>,
    new_sub_slot_iters: Option<u64>,
    challenge_merkle_root: Option<Bytes32>, // MMR root of all challenge chain hashes in this sub-epoch (None for pre-fork)
}

#[cfg_attr(feature = "py-bindings", pymethods)]
impl SubEpochSummaryOld {
    // Convert old format to new format (with None for challenge_merkle_root)
    pub fn to_new(&self) -> SubEpochSummary {
        SubEpochSummary {
            prev_subepoch_summary_hash: self.prev_subepoch_summary_hash,
            reward_chain_hash: self.reward_chain_hash,
            num_blocks_overflow: self.num_blocks_overflow,
            new_difficulty: self.new_difficulty,
            new_sub_slot_iters: self.new_sub_slot_iters,
            challenge_merkle_root: None, // None for pre-fork summaries
        }
    }
}

#[cfg_attr(feature = "py-bindings", pymethods)]
impl SubEpochSummary {
    pub fn compute_hash(&self) -> Bytes32 {
        if self.challenge_merkle_root.is_none() {
            // Pre-fork: convert to old format and use its hash
            let old: SubEpochSummaryOld = self.to_old();
            Bytes32::new(old.hash())
        } else {
            // Post-fork: standard streamable hash (includes all fields)
            Bytes32::new(self.hash())
        }
    }

    fn to_old(&self) -> SubEpochSummaryOld {
        SubEpochSummaryOld {
            prev_subepoch_summary_hash: self.prev_subepoch_summary_hash,
            reward_chain_hash: self.reward_chain_hash,
            num_blocks_overflow: self.num_blocks_overflow,
            new_difficulty: self.new_difficulty,
            new_sub_slot_iters: self.new_sub_slot_iters,
        }
    }
}
