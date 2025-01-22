
use chia_protocol::Bytes32;
use chia_sha2::Sha256;
use std::convert::TryInto;
use crate::pos_quality::expected_plot_size;


#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
pub fn is_overflow_block(num_sps_sub_slot: u32, num_sp_intervals_extra: u8, signage_point_index: u8) -> pyo3::PyResult<bool> {
    if signage_point_index as u32 >= num_sps_sub_slot {
        return Err(pyo3::exceptions::PyValueError::new_err("SP index too high"));
    } else {
        return Ok(signage_point_index as u32 >= num_sps_sub_slot - num_sp_intervals_extra as u32)
    }
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
pub fn calculate_sp_interval_iters(num_sps_sub_slot: u32, sub_slot_iters: u64) -> pyo3::PyResult<u64> {
    if sub_slot_iters % num_sps_sub_slot as u64 == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err("ssi % num_sps_sub_slot == 0"));
    }
    Ok(sub_slot_iters / num_sps_sub_slot as u64)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
pub fn calculate_sp_iters(num_sps_sub_slot: u32, signage_point_index: u8, sub_slot_iters: u64) -> pyo3::PyResult<u64> {
    if signage_point_index as u32 >= num_sps_sub_slot {
        return Err(pyo3::exceptions::PyValueError::new_err("SP index too high"));
    }
    Ok(calculate_sp_interval_iters(num_sps_sub_slot, sub_slot_iters)? * signage_point_index as u64)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
pub fn calculate_ip_iters(
    num_sps_sub_slot: u32,
    signage_point_index: u8,
    num_sp_intervals_extra: u8,
    sub_slot_iters: u64,
    required_iters: u64,
) -> pyo3::PyResult<u64> {
    let sp_interval_iters = calculate_sp_interval_iters(num_sps_sub_slot, sub_slot_iters)?;
    let sp_iters = calculate_sp_iters(num_sps_sub_slot, signage_point_index, sub_slot_iters)?;
    if sp_iters % sp_interval_iters != 0 || sp_iters > sub_slot_iters {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Invalid sp iters {sp_iters} for this ssi {sub_slot_iters}",
        )));
    } else if required_iters >= sp_interval_iters || required_iters == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Required iters {} is not below the sp interval iters {} {} or not >=0",
            required_iters, sp_interval_iters, sub_slot_iters
        )));
    }
    Ok(
        (sp_iters + num_sp_intervals_extra as u64 * sp_interval_iters + required_iters)
            % sub_slot_iters,
    )
}

// TODO: enable and fix below

// #[cfg(feature = "py-bindings")]
// #[pyo3::pyfunction]
// pub fn calculate_iterations_quality(
//     difficulty_constant_factor: u128,
//     quality_string: Bytes32,
//     size: u32,
//     difficulty: u64,
//     cc_sp_output_hash: Bytes32,
// ) -> pyo3::PyResult<u64> {
//     // Hash the concatenation of `quality_string` and `cc_sp_output_hash`
//     let mut hasher = Sha256::new();
//     hasher.update(quality_string);
//     hasher.update(cc_sp_output_hash);
//     let sp_quality_string = hasher.finalize();
    
//     // Convert the hash bytes to a big-endian u128 integer
//     let sp_quality_value = u128::from_be_bytes(sp_quality_string[..16]);

//     // Expected plot size calculation function
//     let plot_size = expected_plot_size(size);

//     // Calculate the number of iterations
//     let iters = (difficulty as u128 * difficulty_constant_factor * sp_quality_value)
//         / ((1_u128 << 256) * plot_size as u128);

//     Ok(iters.max(1) as u64)
// }