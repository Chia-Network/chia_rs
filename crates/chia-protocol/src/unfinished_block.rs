use chia_streamable_macro::streamable;

use crate::Bytes32;
use crate::EndOfSubSlotBundle;
use crate::Program;
use crate::RewardChainBlockUnfinished;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::Streamable;

#[streamable]
pub struct UnfinishedBlock {
    // Full block, without the final VDFs
    finished_sub_slots: Vec<EndOfSubSlotBundle>, // If first sb
    reward_chain_block: RewardChainBlockUnfinished, // Reward chain trunk data
    challenge_chain_sp_proof: Option<VDFProof>,  // If not first sp in sub-slot
    reward_chain_sp_proof: Option<VDFProof>,     // If not first sp in sub-slot
    foliage: Foliage,                            // Reward chain foliage data
    foliage_transaction_block: Option<FoliageTransactionBlock>, // Reward chain foliage data (tx block)
    transactions_info: Option<TransactionsInfo>, // Reward chain foliage data (tx block additional)
    transactions_generator: Option<Program>,     // Program that generates transactions
    transactions_generator_ref_list: Vec<u32>, // List of block heights of previous generators referenced in this block
}

impl UnfinishedBlock {
    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn partial_hash(&self) -> Bytes32 {
        self.reward_chain_block.hash().into()
    }

    pub fn is_transaction_block(&self) -> bool {
        self.foliage.foliage_transaction_block_hash.is_some()
    }

    pub fn total_iters(&self) -> u128 {
        self.reward_chain_block.total_iters
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::ChiaToPython;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl UnfinishedBlock {
    #[getter]
    #[pyo3(name = "prev_header_hash")]
    fn py_prev_header_hash(&self) -> Bytes32 {
        self.prev_header_hash()
    }

    #[getter]
    #[pyo3(name = "partial_hash")]
    fn py_partial_hash(&self) -> Bytes32 {
        self.partial_hash()
    }

    #[pyo3(name = "is_transaction_block")]
    fn py_is_transaction_block(&self) -> bool {
        self.is_transaction_block()
    }

    #[getter]
    #[pyo3(name = "total_iters")]
    fn py_total_iters<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.total_iters(), py)
    }
}
