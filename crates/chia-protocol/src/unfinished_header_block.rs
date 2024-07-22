use chia_streamable_macro::streamable;
use chia_traits::Streamable;

use crate::{
    Bytes, Bytes32, EndOfSubSlotBundle, Foliage, FoliageTransactionBlock,
    RewardChainBlockUnfinished, VDFProof,
};

#[streamable]
pub struct UnfinishedHeaderBlock {
    /// Same as a FullBlock but without TransactionInfo and Generator, used by light clients.
    /// If first sb.
    finished_sub_slots: Vec<EndOfSubSlotBundle>,

    /// Reward chain trunk data.
    reward_chain_block: RewardChainBlockUnfinished,

    /// If not first sp in sub-slot.
    challenge_chain_sp_proof: Option<VDFProof>,

    /// If not first sp in sub-slot.
    reward_chain_sp_proof: Option<VDFProof>,

    /// Reward chain foliage data.
    foliage: Foliage,

    /// Reward chain foliage data (tx block).
    foliage_transaction_block: Option<FoliageTransactionBlock>,

    /// Filter for block transactions.
    transactions_filter: Bytes,
}

impl UnfinishedHeaderBlock {
    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
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
impl UnfinishedHeaderBlock {
    #[getter]
    #[pyo3(name = "prev_header_hash")]
    fn py_prev_header_hash(&self) -> Bytes32 {
        self.prev_header_hash()
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
}
