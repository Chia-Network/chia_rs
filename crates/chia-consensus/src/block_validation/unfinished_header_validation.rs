use std::{collections::HashMap, sync::Arc};

use chia_protocol::{BlockRecord, Bytes32, UnfinishedHeaderBlock};

use crate::{consensus_constants::ConsensusConstants, gen::validation_error::ValidationErr};

pub struct UnfinishedHeaderValidationOptions {
    pub check_filter: bool,
    pub expected_difficulty: u64,
    pub expected_sub_slot_iters: u64,
    pub skip_overflow_last_ss_validation: bool,
    pub skip_vdf_is_valid: bool,
    pub check_sub_epoch_summary: bool,
}

/// Returns required iters.
pub fn validate_unfinished_header_block(
    header_block: UnfinishedHeaderBlock,
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    constants: &ConsensusConstants,
    options: &UnfinishedHeaderValidationOptions,
) -> Result<u64, ValidationErr> {
    todo!()
}
