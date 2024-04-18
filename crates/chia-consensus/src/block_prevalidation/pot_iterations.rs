use crate::{consensus_constants::ConsensusConstants, gen::validation_error::ErrorCode};

pub fn is_overflow_block(
    constants: &ConsensusConstants,
    signage_point_index: u32,
) -> Result<bool, ErrorCode> {
    if signage_point_index >= constants.num_sps_sub_slot {
        return Err(ErrorCode::Unknown);
    }
    return Ok(
        signage_point_index >= constants.num_sps_sub_slot - constants.num_sp_intervals_extra as u32
    );
}
