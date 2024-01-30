use chia_streamable_macro::Streamable;

use crate::streamable_struct;
use crate::Bytes;
use crate::Bytes32;
use crate::EndOfSubSlotBundle;
use crate::RewardChainBlock;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::Streamable;

streamable_struct! (HeaderBlock {
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
});

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

    #[cfg_attr(target_arch = "wasm32", wasm_patch::conv_u128_to_u64_for_wasm)]
    pub fn weight(&self) -> u128 {
        self.reward_chain_block.weight
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
    }

    #[cfg_attr(target_arch = "wasm32", wasm_patch::conv_u128_to_u64_for_wasm)]
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
}

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

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
    fn py_height(&self) -> u32 {
        self.height()
    }

    #[getter]
    #[pyo3(name = "weight")]
    fn py_weight(&self) -> u128 {
        self.weight()
    }

    #[getter]
    #[pyo3(name = "header_hash")]
    fn py_header_hash(&self) -> Bytes32 {
        self.header_hash()
    }

    #[getter]
    #[pyo3(name = "total_iters")]
    fn py_total_iters(&self) -> u128 {
        self.total_iters()
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
}
