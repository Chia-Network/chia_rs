use chia_streamable_macro::streamable;

use crate::{Bytes32, Coin, EndOfSubSlotBundle, GeneratorInfo, Program, RewardChainBlock, VDFProof};
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::chia_error::Result;
use chia_traits::Streamable;

/// Alternative FullBlock implementation using deferred generator parsing.
///
/// **This is an experimental validation type** - it uses `GeneratorInfo` to store
/// generator data as an opaque blob, deferring parsing until access time.
///
/// Wire format is identical to `FullBlock`, but the last two fields are combined
/// into a single blob that's read to EOF. This eliminates parse-time framing overhead
/// (no need to walk the generator structure during deserialization).
///
/// Use this to validate that deferred parsing produces identical results to
/// the current `FullBlock` implementation.
#[streamable]
pub struct FullBlock2 {
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
    
    // Combined field (reads to EOF - everything after transactions_info)
    generator_info: Option<GeneratorInfo>,
}

impl FullBlock2 {
    // Mirror FullBlock API for compatibility

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

    // New API: access generator data via deferred parsing
    
    /// Parse and return the transactions_generator (if present).
    /// 
    /// This is lazily computed from the generator_info blob.
    pub fn transactions_generator(&self) -> Result<Option<Program>> {
        match &self.generator_info {
            Some(info) => {
                let (generator, _) = info.parse_generator_info()?;
                Ok(Some(generator))
            }
            None => Ok(None),
        }
    }

    /// Parse and return the transactions_generator_ref_list.
    /// 
    /// This is lazily computed from the generator_info blob.
    pub fn transactions_generator_ref_list(&self) -> Result<Vec<u32>> {
        match &self.generator_info {
            Some(info) => {
                let (_, ref_list) = info.parse_generator_info()?;
                Ok(ref_list)
            }
            None => Ok(vec![]),
        }
    }

    /// Parse and return both generator and ref_list together (more efficient than calling separately).
    pub fn parse_generator_data(&self) -> Result<(Option<Program>, Vec<u32>)> {
        match &self.generator_info {
            Some(info) => {
                let (generator, ref_list) = info.parse_generator_info()?;
                Ok((Some(generator), ref_list))
            }
            None => Ok((None, vec![])),
        }
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::ChiaToPython;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl FullBlock2 {
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

    #[getter]
    #[pyo3(name = "transactions_generator")]
    fn py_transactions_generator(&self) -> PyResult<Option<Program>> {
        self.transactions_generator()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    #[getter]
    #[pyo3(name = "transactions_generator_ref_list")]
    fn py_transactions_generator_ref_list(&self) -> PyResult<Vec<u32>> {
        self.transactions_generator_ref_list()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}

// Tests are in tests/fullblock_comparison.rs
