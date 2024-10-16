use chia_streamable_macro::streamable;

use crate::Bytes32;

#[streamable]
pub struct SubEpochSummary {
    prev_subepoch_summary_hash: Bytes32,
    reward_chain_hash: Bytes32, // hash of reward chain at end of last segment
    num_blocks_overflow: u8,    // How many more blocks than 384*(N-1)
    new_difficulty: Option<u64>, // Only once per epoch (diff adjustment)
    new_sub_slot_iters: Option<u64>, // Only once per epoch (diff adjustment)
}
