use chia_streamable_macro::streamable;

use crate::Bytes32;
use crate::Coin;
use crate::EndOfSubSlotBundle;
use crate::Program;
use crate::RewardChainBlock;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::Streamable;

#[streamable]
pub struct FullBlock {
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    reward_chain_block: RewardChainBlock,
    challenge_chain_sp_proof: Option<VDFProof>, // # If not first sp in sub-slot
    challenge_chain_ip_proof: VDFProof,
    reward_chain_sp_proof: Option<VDFProof>, // # If not first sp in sub-slot
    reward_chain_ip_proof: VDFProof,
    infused_challenge_chain_ip_proof: Option<VDFProof>, // # Iff deficit < 4
    foliage: Foliage,                                   // # Reward chain foliage data
    foliage_transaction_block: Option<FoliageTransactionBlock>, // # Reward chain foliage data (tx block)
    transactions_info: Option<TransactionsInfo>, // Reward chain foliage data (tx block additional)
    transactions_generator: Option<Program>,     // Program that generates transactions
    transactions_generator_ref_list: Vec<u32>, // List of block heights of previous generators referenced in this block
}

impl FullBlock {
    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
    }

    pub fn is_transaction_block(&self) -> bool {
        self.foliage.foliage_transaction_block_hash.is_some()
    }

    pub fn total_iters(&self) -> u128 {
        self.reward_chain_block.total_iters
    }

    pub fn height(&self) -> u32 {
        self.reward_chain_block.height
    }

    pub fn weight(&self) -> u128 {
        self.reward_chain_block.weight
    }

    pub fn get_included_reward_coins(&self) -> Vec<Coin> {
        if let Some(ti) = &self.transactions_info {
            ti.reward_claims_incorporated.clone()
        } else {
            vec![]
        }
    }

    pub fn is_fully_compactified(&self) -> bool {
        for sub_slot in &self.finished_sub_slots {
            if sub_slot.proofs.challenge_chain_slot_proof.witness_type != 0
                || !sub_slot
                    .proofs
                    .challenge_chain_slot_proof
                    .normalized_to_identity
            {
                return false;
            }
            if let Some(proof) = &sub_slot.proofs.infused_challenge_chain_slot_proof {
                if proof.witness_type != 0 || !proof.normalized_to_identity {
                    return false;
                }
            }
        }

        if let Some(proof) = &self.challenge_chain_sp_proof {
            if proof.witness_type != 0 || !proof.normalized_to_identity {
                return false;
            }
        }
        self.challenge_chain_ip_proof.witness_type == 0
            && self.challenge_chain_ip_proof.normalized_to_identity
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::ChiaToPython;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl FullBlock {
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

    #[pyo3(name = "is_transaction_block")]
    fn py_is_transaction_block(&self) -> bool {
        self.is_transaction_block()
    }

    #[getter]
    #[pyo3(name = "total_iters")]
    fn py_total_iters<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        ChiaToPython::to_python(&self.total_iters(), py)
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

    #[pyo3(name = "get_included_reward_coins")]
    fn py_get_included_reward_coins(&self) -> Vec<Coin> {
        self.get_included_reward_coins()
    }

    #[pyo3(name = "is_fully_compactified")]
    fn py_is_fully_compactified(&self) -> bool {
        self.is_fully_compactified()
    }
}
