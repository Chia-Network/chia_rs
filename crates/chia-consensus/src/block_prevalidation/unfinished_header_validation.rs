use std::{collections::HashMap, sync::Arc};

use chia_protocol::{BlockRecord, Bytes32, UnfinishedHeaderBlock};

use crate::{
    block_prevalidation::pot_iterations::is_overflow_block,
    consensus_constants::ConsensusConstants, gen::validation_error::ErrorCode,
};

pub struct UnfinishedHeaderValidationOptions {
    pub check_filter: bool,
    pub expected_difficulty: u64,
    pub expected_sub_slot_iters: u64,
    pub skip_overflow_last_ss_validation: bool,
    pub skip_vdf_is_valid: bool,
    pub check_sub_epoch_summary: bool,
}

/// Validates an unfinished header block. This is a block without the infusion VDFs (unfinished)
/// and without transactions and transaction info (header). Returns (required_iters, error).
///
/// This method is meant to validate only the unfinished part of the block. However, the finished_sub_slots
/// refers to all sub-slots that were finishes from the previous block's infusion point, up to this blocks
/// infusion point. Therefore, in the case where this is an overflow block, and the last sub-slot is not yet
/// released, header_block.finished_sub_slots will be missing one sub-slot. In this case,
/// skip_overflow_last_ss_validation must be set to True. This will skip validation of end of slots, sub-epochs,
/// and lead to other small tweaks in validation.
pub fn validate_unfinished_header_block(
    header_block: UnfinishedHeaderBlock,
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    constants: &ConsensusConstants,
    options: &UnfinishedHeaderValidationOptions,
) -> Result<u64, ErrorCode> {
    let prev_block = blocks.get(&header_block.prev_header_hash());
    if prev_block.is_none() && header_block.prev_header_hash() != constants.genesis_challenge {
        return Err(ErrorCode::InvalidPrevBlockHash);
    }

    let overflow = is_overflow_block(
        constants,
        header_block.reward_chain_block.signage_point_index as u32,
    );

    todo!()
}

/// Returns true if the missing sub slot was already included in a previous block.
/// Returns False if the sub slot was not included yet, and therefore it is the
/// responsibility of this block to include it.
fn final_eos_is_already_included(
    header_block: UnfinishedHeaderBlock,
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    sub_slot_iters: u64,
) -> bool {
    // We already have an included empty sub slot, which means the prev block is 2 sub slots behind.
    if header_block.finished_sub_slots.len() > 0 {
        return false;
    }

    let mut current = blocks.get(&header_block.prev_header_hash());

    todo!()
}
