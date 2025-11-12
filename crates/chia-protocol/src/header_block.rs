use chia_streamable_macro::streamable;

use crate::unfinished_header_block::UnfinishedHeaderBlock;
use crate::Bytes;
use crate::Bytes32;
use crate::EndOfSubSlotBundle;
use crate::RewardChainBlock;
use crate::RewardChainBlockOld;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::Streamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

// Pre-fork structure (before HARD_FORK2_HEIGHT)
#[streamable]
pub struct HeaderBlockOld {
    // If first sb
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    // Reward chain trunk data
    reward_chain_block: RewardChainBlockOld,
    // If not first sp in sub-slot
    challenge_chain_sp_proof: Option<VDFProof>,
    challenge_chain_ip_proof: VDFProof,
    // If not first sp in sub-slot
    reward_chain_sp_proof: Option<VDFProof>,
    reward_chain_ip_proof: VDFProof,
    // Iff deficit < 4
    infused_challenge_chain_ip_proof: Option<VDFProof>,
    // Reward chain foliage data
    foliage: Foliage,
    // Reward chain foliage data (tx block)
    foliage_transaction_block: Option<FoliageTransactionBlock>,
    // Filter for block transactions
    transactions_filter: Bytes,
    // Reward chain foliage data (tx block additional)
    transactions_info: Option<TransactionsInfo>,
}

// Post-fork structure (at or after HARD_FORK2_HEIGHT) - this is the default
#[streamable]
pub struct HeaderBlock {
    // If first sb
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    // Reward chain trunk data
    reward_chain_block: RewardChainBlock,
    // If not first sp in sub-slot
    challenge_chain_sp_proof: Option<VDFProof>,
    challenge_chain_ip_proof: VDFProof,
    // If not first sp in sub-slot
    reward_chain_sp_proof: Option<VDFProof>,
    reward_chain_ip_proof: VDFProof,
    // Iff deficit < 4
    infused_challenge_chain_ip_proof: Option<VDFProof>,
    // Reward chain foliage data
    foliage: Foliage,
    // Reward chain foliage data (tx block)
    foliage_transaction_block: Option<FoliageTransactionBlock>,
    // Filter for block transactions
    transactions_filter: Bytes,
    // Reward chain foliage data (tx block additional)
    transactions_info: Option<TransactionsInfo>,
}

impl HeaderBlockOld {
    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn prev_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn height(&self) -> u32 {
        self.reward_chain_block.height
    }

    pub fn weight(&self) -> u128 {
        self.reward_chain_block.weight
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
    }

    pub fn total_iters(&self) -> u128 {
        self.reward_chain_block.total_iters
    }

    pub fn log_string(&self) -> String {
        format!(
            "block {:?} sb_height {} ",
            self.header_hash(),
            self.height()
        )
    }

    pub fn is_transaction_block(&self) -> bool {
        self.reward_chain_block.is_transaction_block
    }

    pub fn first_in_sub_slot(&self) -> bool {
        !self.finished_sub_slots.is_empty()
    }

    // Always safe: upgrade reward_chain_block to new version
    pub fn to_new(&self) -> HeaderBlock {
        HeaderBlock {
            finished_sub_slots: self.finished_sub_slots.clone(),
            reward_chain_block: self.reward_chain_block.to_new(),
            challenge_chain_sp_proof: self.challenge_chain_sp_proof.clone(),
            challenge_chain_ip_proof: self.challenge_chain_ip_proof.clone(),
            reward_chain_sp_proof: self.reward_chain_sp_proof.clone(),
            reward_chain_ip_proof: self.reward_chain_ip_proof.clone(),
            infused_challenge_chain_ip_proof: self.infused_challenge_chain_ip_proof.clone(),
            foliage: self.foliage.clone(),
            foliage_transaction_block: self.foliage_transaction_block.clone(),
            transactions_filter: self.transactions_filter.clone(),
            transactions_info: self.transactions_info.clone(),
        }
    }
}

impl HeaderBlock {
    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn prev_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn height(&self) -> u32 {
        self.reward_chain_block.height
    }

    pub fn weight(&self) -> u128 {
        self.reward_chain_block.weight
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
    }

    pub fn total_iters(&self) -> u128 {
        self.reward_chain_block.total_iters
    }

    pub fn log_string(&self) -> String {
        format!(
            "block {:?} sb_height {} ",
            self.header_hash(),
            self.height()
        )
    }

    pub fn is_transaction_block(&self) -> bool {
        self.reward_chain_block.is_transaction_block
    }

    pub fn first_in_sub_slot(&self) -> bool {
        !self.finished_sub_slots.is_empty()
    }

    pub fn into_unfinished_header_block(self) -> UnfinishedHeaderBlock {
        UnfinishedHeaderBlock {
            finished_sub_slots: self.finished_sub_slots,
            reward_chain_block: self.reward_chain_block.get_unfinished(),
            challenge_chain_sp_proof: self.challenge_chain_sp_proof,
            reward_chain_sp_proof: self.reward_chain_sp_proof,
            foliage: self.foliage,
            foliage_transaction_block: self.foliage_transaction_block,
            transactions_filter: self.transactions_filter,
        }
    }

    // Validated downgrade: only safe if reward_chain_block can downgrade
    #[cfg(feature = "py-bindings")]
    pub fn to_old(&self) -> PyResult<HeaderBlockOld> {
        let reward_chain_block_old = self.reward_chain_block.to_old()?;
        Ok(HeaderBlockOld {
            finished_sub_slots: self.finished_sub_slots.clone(),
            reward_chain_block: reward_chain_block_old,
            challenge_chain_sp_proof: self.challenge_chain_sp_proof.clone(),
            challenge_chain_ip_proof: self.challenge_chain_ip_proof.clone(),
            reward_chain_sp_proof: self.reward_chain_sp_proof.clone(),
            reward_chain_ip_proof: self.reward_chain_ip_proof.clone(),
            infused_challenge_chain_ip_proof: self.infused_challenge_chain_ip_proof.clone(),
            foliage: self.foliage.clone(),
            foliage_transaction_block: self.foliage_transaction_block.clone(),
            transactions_filter: self.transactions_filter.clone(),
            transactions_info: self.transactions_info.clone(),
        })
    }

    #[cfg(not(feature = "py-bindings"))]
    pub fn to_old(&self) -> Result<HeaderBlockOld, String> {
        let reward_chain_block_old = self.reward_chain_block.to_old()?;
        Ok(HeaderBlockOld {
            finished_sub_slots: self.finished_sub_slots.clone(),
            reward_chain_block: reward_chain_block_old,
            challenge_chain_sp_proof: self.challenge_chain_sp_proof.clone(),
            challenge_chain_ip_proof: self.challenge_chain_ip_proof.clone(),
            reward_chain_sp_proof: self.reward_chain_sp_proof.clone(),
            reward_chain_ip_proof: self.reward_chain_ip_proof.clone(),
            infused_challenge_chain_ip_proof: self.infused_challenge_chain_ip_proof.clone(),
            foliage: self.foliage.clone(),
            foliage_transaction_block: self.foliage_transaction_block.clone(),
            transactions_filter: self.transactions_filter.clone(),
            transactions_info: self.transactions_info.clone(),
        })
    }

    // Unchecked downgrade: caller guarantees reward_chain_block can downgrade
    pub fn to_old_unchecked(&self) -> HeaderBlockOld {
        HeaderBlockOld {
            finished_sub_slots: self.finished_sub_slots.clone(),
            reward_chain_block: self.reward_chain_block.to_old_unchecked(),
            challenge_chain_sp_proof: self.challenge_chain_sp_proof.clone(),
            challenge_chain_ip_proof: self.challenge_chain_ip_proof.clone(),
            reward_chain_sp_proof: self.reward_chain_sp_proof.clone(),
            reward_chain_ip_proof: self.reward_chain_ip_proof.clone(),
            infused_challenge_chain_ip_proof: self.infused_challenge_chain_ip_proof.clone(),
            foliage: self.foliage.clone(),
            foliage_transaction_block: self.foliage_transaction_block.clone(),
            transactions_filter: self.transactions_filter.clone(),
            transactions_info: self.transactions_info.clone(),
        }
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::ChiaToPython;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl HeaderBlockOld {
    #[getter]
    #[pyo3(name = "prev_header_hash")]
    fn py_prev_header_hash(&self) -> Bytes32 {
        self.prev_header_hash()
    }

    #[getter]
    #[pyo3(name = "prev_hash")]
    fn py_prev_hash(&self) -> Bytes32 {
        self.prev_hash()
    }

    #[getter]
    #[pyo3(name = "height")]
    fn py_height<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.height(), py)
    }

    #[getter]
    #[pyo3(name = "weight")]
    fn py_weight<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.weight(), py)
    }

    #[getter]
    #[pyo3(name = "header_hash")]
    fn py_header_hash(&self) -> Bytes32 {
        self.header_hash()
    }

    #[getter]
    #[pyo3(name = "total_iters")]
    fn py_total_iters<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.total_iters(), py)
    }

    #[getter]
    #[pyo3(name = "log_string")]
    fn py_log_string(&self) -> String {
        self.log_string()
    }

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

    #[pyo3(name = "to_new")]
    fn py_to_new(&self) -> HeaderBlock {
        self.to_new()
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl HeaderBlock {
    #[getter]
    #[pyo3(name = "prev_header_hash")]
    fn py_prev_header_hash(&self) -> Bytes32 {
        self.prev_header_hash()
    }

    #[getter]
    #[pyo3(name = "prev_hash")]
    fn py_prev_hash(&self) -> Bytes32 {
        self.prev_hash()
    }

    #[getter]
    #[pyo3(name = "height")]
    fn py_height<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.height(), py)
    }

    #[getter]
    #[pyo3(name = "weight")]
    fn py_weight<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.weight(), py)
    }

    #[getter]
    #[pyo3(name = "header_hash")]
    fn py_header_hash(&self) -> Bytes32 {
        self.header_hash()
    }

    #[getter]
    #[pyo3(name = "total_iters")]
    fn py_total_iters<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.total_iters(), py)
    }

    #[getter]
    #[pyo3(name = "log_string")]
    fn py_log_string(&self) -> String {
        self.log_string()
    }

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

    #[pyo3(name = "to_old")]
    fn py_to_old(&self) -> PyResult<HeaderBlockOld> {
        self.to_old()
    }
}
