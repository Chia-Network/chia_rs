use chia_traits::chia_error::{Error, Result};

fn add_catch_overflow(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(Error::InvalidPotIteration)
}

fn mult_catch_overflow(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or(Error::InvalidPotIteration)
}

fn sub_catch_underflow(a: u32, b: u32) -> Result<u32> {
    a.checked_sub(b).ok_or(Error::InvalidPotIteration)
}

fn mod_catch_error(a: u64, b: u64) -> Result<u64> {
    a.checked_rem(b).ok_or(Error::InvalidPotIteration)
}

fn div_catch_error(a: u64, b: u64) -> Result<u64> {
    a.checked_div(b).ok_or(Error::InvalidPotIteration)
}

pub fn is_overflow_block(
    num_sps_sub_slot: u32,
    num_sp_intervals_extra: u8,
    signage_point_index: u32,
) -> Result<bool> {
    if signage_point_index >= num_sps_sub_slot {
        return Err(Error::InvalidPotIteration);
    }
    Ok(
        signage_point_index
            >= sub_catch_underflow(num_sps_sub_slot, num_sp_intervals_extra as u32)?,
    )
}

pub fn calculate_sp_interval_iters(num_sps_sub_slot: u32, sub_slot_iters: u64) -> Result<u64> {
    if mod_catch_error(sub_slot_iters, num_sps_sub_slot as u64)? != 0 {
        return Err(Error::InvalidPotIteration);
    }
    div_catch_error(sub_slot_iters, num_sps_sub_slot as u64)
}

pub fn calculate_sp_iters(
    num_sps_sub_slot: u32,
    sub_slot_iters: u64,
    signage_point_index: u32,
) -> Result<u64> {
    if signage_point_index >= num_sps_sub_slot {
        return Err(Error::InvalidPotIteration);
    }
    mult_catch_overflow(
        calculate_sp_interval_iters(num_sps_sub_slot, sub_slot_iters)?,
        signage_point_index as u64,
    )
}

pub fn calculate_ip_iters(
    num_sps_sub_slot: u32,
    num_sp_intervals_extra: u8,
    sub_slot_iters: u64,
    signage_point_index: u32,
    required_iters: u64,
) -> Result<u64> {
    let sp_interval_iters = calculate_sp_interval_iters(num_sps_sub_slot, sub_slot_iters)?;
    let sp_iters = calculate_sp_iters(num_sps_sub_slot, sub_slot_iters, signage_point_index)?;
    if mod_catch_error(sp_iters, sp_interval_iters)? != 0
        || sp_iters > sub_slot_iters
        || required_iters >= sp_interval_iters
        || required_iters == 0
    {
        return Err(Error::InvalidPotIteration);
    }
    mod_catch_error(
        add_catch_overflow(
            add_catch_overflow(
                sp_iters,
                mult_catch_overflow(num_sp_intervals_extra as u64, sp_interval_iters)?,
            )?,
            required_iters,
        )?,
        sub_slot_iters,
    )
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
#[pyo3(name = "is_overflow_block")]
pub fn py_is_overflow_block(
    num_sps_sub_slot: u32,
    num_sp_intervals_extra: u8,
    signage_point_index: u32,
) -> pyo3::PyResult<bool> {
    Ok(is_overflow_block(
        num_sps_sub_slot,
        num_sp_intervals_extra,
        signage_point_index,
    )?)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
#[pyo3(name = "calculate_sp_interval_iters")]
pub fn py_calculate_sp_interval_iters(
    num_sps_sub_slot: u32,
    sub_slot_iters: u64,
) -> pyo3::PyResult<u64> {
    Ok(calculate_sp_interval_iters(
        num_sps_sub_slot,
        sub_slot_iters,
    )?)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
#[pyo3(name = "calculate_sp_iters")]
pub fn py_calculate_sp_iters(
    num_sps_sub_slot: u32,
    sub_slot_iters: u64,
    signage_point_index: u32,
) -> pyo3::PyResult<u64> {
    Ok(calculate_sp_iters(
        num_sps_sub_slot,
        sub_slot_iters,
        signage_point_index,
    )?)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
#[pyo3(name = "calculate_ip_iters")]
pub fn py_calculate_ip_iters(
    num_sps_sub_slot: u32,
    num_sp_intervals_extra: u8,
    sub_slot_iters: u64,
    signage_point_index: u32,
    required_iters: u64,
) -> pyo3::PyResult<u64> {
    Ok(calculate_ip_iters(
        num_sps_sub_slot,
        num_sp_intervals_extra,
        sub_slot_iters,
        signage_point_index,
        required_iters,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    static NUM_SPS_SUB_SLOT: u32 = 32;
    static NUM_SP_INTERVALS_EXTRA: u8 = 3;

    #[test]
    fn test_is_overflow_block() {
        assert!(
            !is_overflow_block(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, 27)
                .expect("valid SP index")
        );
        assert!(
            !is_overflow_block(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, 28)
                .expect("valid SP index")
        );
        assert!(
            is_overflow_block(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, 29)
                .expect("valid SP index")
        );
        assert!(
            is_overflow_block(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, 30)
                .expect("valid SP index")
        );
        assert!(
            is_overflow_block(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, 31)
                .expect("valid SP index")
        );
        assert!(is_overflow_block(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, 32).is_err());
    }

    #[test]
    fn test_calculate_sp_iters() {
        let ssi: u64 = 100_001 * 64 * 4;
        assert!(calculate_sp_iters(NUM_SPS_SUB_SLOT, ssi, 32).is_err());
        calculate_sp_iters(NUM_SPS_SUB_SLOT, ssi, 31).expect("valid_result");
    }

    #[test]
    fn test_calculate_ip_iters() {
        // # num_sps_sub_slot: u32,
        // # num_sp_intervals_extra: u8,
        // # sub_slot_iters: u64,
        // # signage_point_index: u8,
        // # required_iters: u64,
        let ssi: u64 = 100_001 * 64 * 4;
        let sp_interval_iters = ssi / NUM_SPS_SUB_SLOT as u64;

        // Invalid signage point index
        assert!(
            calculate_ip_iters(NUM_SPS_SUB_SLOT, NUM_SP_INTERVALS_EXTRA, ssi, 123, 100_000)
                .is_err()
        );

        let sp_iters = sp_interval_iters * 13;

        // required_iters too high
        assert!(calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            sp_interval_iters.try_into().unwrap(),
            sp_interval_iters
        )
        .is_err());

        // required_iters too high
        assert!(calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            sp_interval_iters.try_into().unwrap(),
            sp_interval_iters * 12
        )
        .is_err());

        // required_iters too low (0)
        assert!(calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            sp_interval_iters.try_into().unwrap(),
            0
        )
        .is_err());

        let required_iters = sp_interval_iters - 1;
        let ip_iters = calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            13,
            required_iters,
        )
        .expect("should be valid");
        assert_eq!(
            ip_iters,
            sp_iters + (NUM_SP_INTERVALS_EXTRA as u64 * sp_interval_iters) + required_iters
        );

        let required_iters = 1_u64;
        let ip_iters = calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            13,
            required_iters,
        )
        .expect("valid");
        assert_eq!(
            ip_iters,
            sp_iters + (NUM_SP_INTERVALS_EXTRA as u64 * sp_interval_iters) + required_iters
        );

        let required_iters: u64 = ssi * 4 / 300;
        let ip_iters = calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            13,
            required_iters,
        )
        .expect("valid");
        assert_eq!(
            ip_iters,
            sp_iters + (NUM_SP_INTERVALS_EXTRA as u64 * sp_interval_iters) + required_iters
        );
        assert!(sp_iters < ip_iters);

        // Overflow
        let sp_iters = sp_interval_iters * (NUM_SPS_SUB_SLOT - 1) as u64;
        let ip_iters = calculate_ip_iters(
            NUM_SPS_SUB_SLOT,
            NUM_SP_INTERVALS_EXTRA,
            ssi,
            NUM_SPS_SUB_SLOT - 1_u32,
            required_iters,
        )
        .expect("valid");
        assert_eq!(
            ip_iters,
            (sp_iters + (NUM_SP_INTERVALS_EXTRA as u64 * sp_interval_iters) + required_iters) % ssi
        );
        assert!(sp_iters > ip_iters);
    }
}
