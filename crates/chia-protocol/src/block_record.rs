use crate::{calculate_ip_iters, calculate_sp_iters};
use crate::{Bytes32, ClassgroupElement, Coin, SubEpochSummary};
use chia_streamable_macro::streamable;
use chia_traits::chia_error::Result;
#[cfg(feature = "py-bindings")]
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

    pub fn sp_iters_impl(&self, num_sps_sub_slot: u8) -> Result<u64> {
        calculate_sp_iters(
            num_sps_sub_slot,
            self.sub_slot_iters,
            self.signage_point_index,
        )
    }

    pub fn ip_iters_impl(&self, num_sps_sub_slot: u8, num_sp_intervals_extra: u8) -> Result<u64> {
        calculate_ip_iters(
            num_sps_sub_slot,
            num_sp_intervals_extra,
            self.sub_slot_iters,
            self.signage_point_index,
            self.required_iters,
        )
    }
}

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

    #[pyo3(name = "ip_sub_slot_total_iters")]
    fn ip_sub_slot_total_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        self.total_iters
            .checked_sub(self.py_ip_iters_impl(constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))
    }

    #[pyo3(name = "sp_iters")]
    fn py_sp_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u64> {
        let num_sps_sub_slot = constants.getattr("NUM_SPS_SUB_SLOT")?.extract::<u8>()?;
        self.sp_iters_impl(num_sps_sub_slot).map_err(Into::into)
    }

    #[pyo3(name = "ip_iters")]
    fn py_ip_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u64> {
        let num_sps_sub_slot = constants.getattr("NUM_SPS_SUB_SLOT")?.extract::<u8>()?;
        let num_sp_intervals_extra = constants
            .getattr("NUM_SP_INTERVALS_EXTRA")?
            .extract::<u8>()?;
        self.ip_iters_impl(num_sps_sub_slot, num_sp_intervals_extra)
            .map_err(Into::into)
    }

    #[pyo3(name = "sp_sub_slot_total_iters")]
    fn sp_sub_slot_total_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        let ret = self
            .total_iters
            .checked_sub(self.py_ip_iters_impl(constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))?;
        if self.overflow {
            ret.checked_sub(self.sub_slot_iters as u128)
                .ok_or(PyValueError::new_err("uint128 overflow"))
        } else {
            Ok(ret)
        }
    }

    #[pyo3(name = "sp_total_iters")]
    fn sp_total_iters_impl(&self, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        self.sp_sub_slot_total_iters_impl(constants)?
            .checked_add(self.py_sp_iters_impl(constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))
    }
}
