use chia_streamable_macro::streamable;

use crate::{Bytes32, ClassgroupElement, Coin, SubEpochSummary};

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
}

#[cfg(feature = "py-bindings")]
use pyo3::types::PyDict;

#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyValueError;

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

    // TODO: at some point it would be nice to port
    // chia.consensus.pot_iterations to rust, and make this less hacky
    fn sp_sub_slot_total_iters_impl(
        &self,
        py: Python<'_>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<u128> {
        let ret = self
            .total_iters
            .checked_sub(self.ip_iters_impl(py, constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))?;
        if self.overflow {
            ret.checked_sub(self.sub_slot_iters as u128)
                .ok_or(PyValueError::new_err("uint128 overflow"))
        } else {
            Ok(ret)
        }
    }

    fn ip_sub_slot_total_iters_impl(
        &self,
        py: Python<'_>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<u128> {
        self.total_iters
            .checked_sub(self.ip_iters_impl(py, constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))
    }

    fn sp_iters_impl(&self, py: Python<'_>, constants: &Bound<'_, PyAny>) -> PyResult<u64> {
        let ctx = PyDict::new_bound(py);
        ctx.set_item("sub_slot_iters", self.sub_slot_iters)?;
        ctx.set_item("signage_point_index", self.signage_point_index)?;
        ctx.set_item("constants", constants)?;
        py.run_bound(
            "from chia.consensus.pot_iterations import calculate_ip_iters, calculate_sp_iters\n\
            ret = calculate_sp_iters(constants, sub_slot_iters, signage_point_index)\n",
            None,
            Some(&ctx),
        )?;
        ctx.get_item("ret").unwrap().unwrap().extract::<u64>()
    }

    fn ip_iters_impl(&self, py: Python<'_>, constants: &Bound<'_, PyAny>) -> PyResult<u64> {
        let ctx = PyDict::new_bound(py);
        ctx.set_item("sub_slot_iters", self.sub_slot_iters)?;
        ctx.set_item("signage_point_index", self.signage_point_index)?;
        ctx.set_item("required_iters", self.required_iters)?;
        ctx.set_item("constants", constants)?;
        py.run_bound(
            "from chia.consensus.pot_iterations import calculate_ip_iters, calculate_sp_iters\n\
            ret = calculate_ip_iters(constants, sub_slot_iters, signage_point_index, required_iters)\n",
            None,
            Some(&ctx),
            )?;
        ctx.get_item("ret").unwrap().unwrap().extract::<u64>()
    }

    fn sp_total_iters_impl(&self, py: Python<'_>, constants: &Bound<'_, PyAny>) -> PyResult<u128> {
        self.sp_sub_slot_total_iters_impl(py, constants)?
            .checked_add(self.sp_iters_impl(py, constants)? as u128)
            .ok_or(PyValueError::new_err("uint128 overflow"))
    }

    fn sp_sub_slot_total_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.sp_sub_slot_total_iters_impl(py, constants)?, py)
    }

    fn ip_sub_slot_total_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.ip_sub_slot_total_iters_impl(py, constants)?, py)
    }

    fn sp_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.sp_iters_impl(py, constants)?, py)
    }

    fn ip_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.ip_iters_impl(py, constants)?, py)
    }

    fn sp_total_iters<'a>(
        &self,
        py: Python<'a>,
        constants: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.sp_total_iters_impl(py, constants)?, py)
    }
}
