use chia_streamable_macro::streamable;

use crate::{
    Bytes32, Coin, EndOfSubSlotBundle, GeneratorInfo, Program, RewardChainBlock, VDFProof,
};
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::Streamable;
use chia_traits::chia_error::Result;

/// Full block with deferred generator parsing.
///
/// Wire format is identical to the original `FullBlock`, but the final
/// `transactions_generator` and `transactions_generator_ref_list` fields are
/// stored as a single opaque tail blob. Accessors parse that tail lazily.
#[streamable]
pub struct FullBlock {
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    reward_chain_block: RewardChainBlock,
    challenge_chain_sp_proof: Option<VDFProof>,
    challenge_chain_ip_proof: VDFProof,
    reward_chain_sp_proof: Option<VDFProof>,
    reward_chain_ip_proof: VDFProof,
    infused_challenge_chain_ip_proof: Option<VDFProof>,
    foliage: Foliage,
    foliage_transaction_block: Option<FoliageTransactionBlock>,
    transactions_info: Option<TransactionsInfo>,

    // Combined tail (reads to EOF - everything after transactions_info)
    #[cfg_attr(feature = "py-bindings", py_api_flatten)]
    generator_info: GeneratorInfo,
}

impl FullBlock {
    #[allow(clippy::too_many_arguments)]
    pub fn from_generator_parts(
        finished_sub_slots: Vec<EndOfSubSlotBundle>,
        reward_chain_block: RewardChainBlock,
        challenge_chain_sp_proof: Option<VDFProof>,
        challenge_chain_ip_proof: VDFProof,
        reward_chain_sp_proof: Option<VDFProof>,
        reward_chain_ip_proof: VDFProof,
        infused_challenge_chain_ip_proof: Option<VDFProof>,
        foliage: Foliage,
        foliage_transaction_block: Option<FoliageTransactionBlock>,
        transactions_info: Option<TransactionsInfo>,
        transactions_generator: Option<Program>,
        transactions_generator_ref_list: Vec<u32>,
    ) -> Self {
        Self {
            finished_sub_slots,
            reward_chain_block,
            challenge_chain_sp_proof,
            challenge_chain_ip_proof,
            reward_chain_sp_proof,
            reward_chain_ip_proof,
            infused_challenge_chain_ip_proof,
            foliage,
            foliage_transaction_block,
            transactions_info,
            generator_info: GeneratorInfo::from_parts(
                transactions_generator,
                transactions_generator_ref_list,
            ),
        }
    }

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

    /// Parse and return the transactions generator if present.
    pub fn transactions_generator(&self) -> Result<Option<Program>> {
        let (generator, _) = self.generator_info.parse_generator_info()?;
        Ok(generator)
    }

    /// Parse and return the transactions generator ref list.
    pub fn transactions_generator_ref_list(&self) -> Result<Vec<u32>> {
        let (_, ref_list) = self.generator_info.parse_generator_info()?;
        Ok(ref_list)
    }

    /// Parse and return both generator fields in one pass.
    pub fn parse_generator_data(&self) -> Result<(Option<Program>, Vec<u32>)> {
        self.generator_info.parse_generator_info()
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
