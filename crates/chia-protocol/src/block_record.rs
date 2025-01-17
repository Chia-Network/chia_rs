use crate::{Bytes32, ClassgroupElement, Coin, SubEpochSummary};
use chia_streamable_macro::streamable;
use pyo3::exceptions::PyValueError;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

// This class is not included or hashed into the blockchain, but it is kept in memory as a more
// efficient way to maintain data about the blockchain. This allows us to validate future blocks,
// difficulty adjustments, etc, without saving the whole header block in memory.
#[streamable]
pub struct BlockRecord {
    header_hash: Bytes32,
    // Header hash of the previous block
    prev_hash: Bytes32,
    height: u32,
    // Total cumulative difficulty of all ancestor blocks since genesis
    weight: u128,
    // Total number of VDF iterations since genesis, including this block
    total_iters: u128,
    signage_point_index: u8,
    // This is the intermediary VDF output at ip_iters in challenge chain
    challenge_vdf_output: ClassgroupElement,
    // This is the intermediary VDF output at ip_iters in infused cc, iff deficit <= 3
    infused_challenge_vdf_output: Option<ClassgroupElement>,
    // The reward chain infusion output, input to next VDF
    reward_infusion_new_challenge: Bytes32,
    // Hash of challenge chain data, used to validate end of slots in the future
    challenge_block_info_hash: Bytes32,
    // Current network sub_slot_iters parameter
    sub_slot_iters: u64,
    // Need to keep track of these because Coins are created in a future block
    pool_puzzle_hash: Bytes32,
    farmer_puzzle_hash: Bytes32,
    // The number of iters required for this proof of space
    required_iters: u64,
    // A deficit of 16 is an overflow block after an infusion. Deficit of 15 is a challenge block
    deficit: u8,
    overflow: bool,
    prev_transaction_block_height: u32,

    // Transaction block (present iff is_transaction_block)
    timestamp: Option<u64>,
    // Header hash of the previous transaction block
    prev_transaction_block_hash: Option<Bytes32>,
    fees: Option<u64>,
    reward_claims_incorporated: Option<Vec<Coin>>,

    // Slot (present iff this is the first SB in sub slot)
    finished_challenge_slot_hashes: Option<Vec<Bytes32>>,
    finished_infused_challenge_slot_hashes: Option<Vec<Bytes32>>,
    finished_reward_slot_hashes: Option<Vec<Bytes32>>,

    // Sub-epoch (present iff this is the first SB after sub-epoch)
    sub_epoch_summary_included: Option<SubEpochSummary>,
}

impl BlockRecord {
    pub fn is_transaction_block(&self) -> bool {
        self.timestamp.is_some()
    }

    pub fn first_in_sub_slot(&self) -> bool {
        self.finished_challenge_slot_hashes.is_some()
    }

    pub fn is_challenge_block(&self, min_blocks_per_challenge_block: u8) -> bool {
        self.deficit == min_blocks_per_challenge_block - 1
    }

    fn calculate_sp_interval_iters(&self, num_sps_sub_slot: u64) -> PyResult<u64> {
        if self.sub_slot_iters % num_sps_sub_slot != 0 {
            return Err(PyValueError::new_err(
                "sub_slot_iters % constants.NUM_SPS_SUB_SLOT != 0",
            ));
        }
        Ok(self.sub_slot_iters / num_sps_sub_slot)
    }

    fn calculate_sp_iters(&self, num_sps_sub_slot: u32) -> PyResult<u64> {
        if self.signage_point_index as u32 >= num_sps_sub_slot {
            return Err(PyValueError::new_err("SP index too high"));
        }
        Ok(self.calculate_sp_interval_iters(num_sps_sub_slot as u64)?
            * self.signage_point_index as u64)
    }

    fn calculate_ip_iters(
        &self,
        num_sps_sub_slot: u32,
        num_sp_intervals_extra: u8,
    ) -> PyResult<u64> {
        let sp_iters = self.calculate_sp_iters(num_sps_sub_slot)?;
        let sp_interval_iters = self.calculate_sp_interval_iters(num_sps_sub_slot as u64)?;
        if sp_iters % sp_interval_iters != 0 || sp_iters >= self.sub_slot_iters {
            return Err(PyValueError::new_err(format!(
                "Invalid sp iters {sp_iters} for this ssi {}",
                self.sub_slot_iters
            )));
        } else if self.required_iters >= sp_interval_iters || self.required_iters == 0 {
            return Err(PyValueError::new_err(format!(
                "Required iters {} is not below the sp interval iters {} {} or not >=0",
                self.required_iters, sp_interval_iters, self.sub_slot_iters
            )));
        }
        Ok(
            (sp_iters + num_sp_intervals_extra as u64 * sp_interval_iters + self.required_iters)
                % self.sub_slot_iters,
        )
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::ChiaToPython;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl BlockRecord {
    #[getter]
    #[pyo3(name = "is_transaction_block")]
    fn py_is_transaction_block(&self) -> bool {
        self.is_transaction_block()
    }

    #[getter]
    #[pyo3(name = "first_in_sub_slot")]
    fn py_first_in_sub_slot(&self) -> bool {
        self.first_in_sub_slot()
    }

    #[pyo3(name = "is_challenge_block")]
    fn py_is_challenge_block(&self, constants: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.is_challenge_block(
            constants
                .getattr("MIN_BLOCKS_PER_CHALLENGE_BLOCK")?
                .extract::<u8>()?,
        ))
    }

    // TODO: these could be implemented as a total port of pot iterations
    fn sp_sub_slot_total_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        let ret = self
            .total_iters
            .checked_sub(self.ip_iters_impl(constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))?;
        if self.overflow {
            ret.checked_sub(self.sub_slot_iters as u128)
                .ok_or(PyValueError::new_err("uint128 overflow"))
        } else {
            Ok(ret)
        }
    }

    fn ip_sub_slot_total_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        self.total_iters
            .checked_sub(self.ip_iters_impl(constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))
    }

    fn sp_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u64> {
        let num_sps_sub_slot = constants.get_item("NUM_SPS_SUB_SLOT")?.extract::<u32>()?;
        self.calculate_sp_iters(num_sps_sub_slot)
    }

    fn ip_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u64> {
        let num_sps_sub_slot = constants.get_item("NUM_SPS_SUB_SLOT")?.extract::<u32>()?;
        let num_sp_intervals_extra = constants
            .get_item("NUM_SP_INTERVALS_EXTRA")?
            .extract::<u8>()?;
        self.calculate_ip_iters(num_sps_sub_slot, num_sp_intervals_extra)
    }

    fn sp_total_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        self.sp_sub_slot_total_iters_impl(constants)?
            .checked_add(self.sp_iters_impl(constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))
    }

    fn sp_sub_slot_total_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.sp_sub_slot_total_iters_impl(constants)?, py)
    }

    fn ip_sub_slot_total_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.ip_sub_slot_total_iters_impl(constants)?, py)
    }

    fn sp_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.sp_iters_impl(constants)?, py)
    }

    fn ip_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.ip_iters_impl(constants)?, py)
    }

    fn sp_total_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.sp_total_iters_impl(constants)?, py)
    }
}
