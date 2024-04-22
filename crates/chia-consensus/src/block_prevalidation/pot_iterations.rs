use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum PotIterationsError {
    #[error("SP index too high")]
    SpIndexTooHigh,

    #[error("Invalid SP iters {sp_iters} for this SSI {sub_slot_iters}")]
    InvalidSpIters { sp_iters: u64, sub_slot_iters: u64 },

    #[error("Required iters {required_iters} is not below the SP interval iters {sp_interval_iters} {sub_slot_iters} or not >0.")]
    RequiredNotBelowSpInterval {
        required_iters: u64,
        sp_interval_iters: u64,
        sub_slot_iters: u64,
    },
}

pub fn calculate_sp_interval_iters(constants: &ConsensusConstants, sub_slot_iters: u64) -> u64 {
    assert!(sub_slot_iters % constants.num_sps_sub_slot as u64 == 0);
    sub_slot_iters / constants.num_sps_sub_slot as u64
}

/// Returns `None` if the sp index is too high.
pub fn calculate_sp_iters(
    constants: &ConsensusConstants,
    sub_slot_iters: u64,
    signage_point_index: u8,
) -> Result<u64, PotIterationsError> {
    if signage_point_index as u32 >= constants.num_sps_sub_slot {
        return Err(PotIterationsError::SpIndexTooHigh);
    }
    Ok(calculate_sp_interval_iters(constants, sub_slot_iters) * signage_point_index as u64)
}

/// Note that the SSI is for the block passed in, which might be in the previous epoch.
pub fn calculate_ip_iters(
    constants: &ConsensusConstants,
    sub_slot_iters: u64,
    signage_point_index: u8,
    required_iters: u64,
) -> Result<u64, PotIterationsError> {
    // return uint64((sp_iters + constants.NUM_SP_INTERVALS_EXTRA * sp_interval_iters + required_iters) % sub_slot_iters)

    let sp_iters = calculate_sp_iters(constants, sub_slot_iters, signage_point_index)?;
    let sp_interval_iters = calculate_sp_interval_iters(constants, sub_slot_iters);

    if sp_iters % sp_interval_iters != 0 || sp_iters >= sub_slot_iters {
        return Err(PotIterationsError::InvalidSpIters {
            sp_iters,
            sub_slot_iters,
        });
    }

    if required_iters >= sp_interval_iters || required_iters == 0 {
        return Err(PotIterationsError::RequiredNotBelowSpInterval {
            required_iters,
            sp_interval_iters,
            sub_slot_iters,
        });
    }

    Ok(
        (sp_iters + constants.num_sp_intervals_extra as u64 * sp_interval_iters + required_iters)
            % sub_slot_iters,
    )
}
