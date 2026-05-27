use chia_sha2::Sha256;
use chia_streamable_macro::streamable;

use crate::Bytes;
use crate::Bytes32;
use crate::Coin;
use crate::EndOfSubSlotBundle;
use crate::Program;
use crate::RewardChainBlock;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::Streamable;
use chia_traits::chia_error::Result;
use std::io::Cursor;

// Similar to ProofOfSpace, we use unused bits in the Optional<> prefix byte
// for transactions_generator to encode a version flag. Bit 1 (0b10) indicates
// the "raw bytes" format where the generator is serialized as length-prefixed
// bytes (like Bytes) instead of a self-describing CLVM Program, and
// transactions_generator_ref_list is omitted entirely.
#[streamable(no_streamable)]
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

    // Raw generator bytes, only used when version == 1. Mutually exclusive
    // with transactions_generator and transactions_generator_ref_list.
    transactions_generator_buffer: Option<Vec<u8>>,

    // 0 = legacy format (Program serialization + ref_list)
    // 1 = raw bytes format (length-prefixed bytes, ref_list omitted)
    version: u8,
}

impl Streamable for FullBlock {
    fn update_digest(&self, digest: &mut Sha256) {
        self.finished_sub_slots.update_digest(digest);
        self.reward_chain_block.update_digest(digest);
        self.challenge_chain_sp_proof.update_digest(digest);
        self.challenge_chain_ip_proof.update_digest(digest);
        self.reward_chain_sp_proof.update_digest(digest);
        self.reward_chain_ip_proof.update_digest(digest);
        self.infused_challenge_chain_ip_proof.update_digest(digest);
        self.foliage.update_digest(digest);
        self.foliage_transaction_block.update_digest(digest);
        self.transactions_info.update_digest(digest);

        if self.version == 0 {
            self.transactions_generator.update_digest(digest);
            self.transactions_generator_ref_list.update_digest(digest);
        } else {
            match &self.transactions_generator_buffer {
                None => {
                    0b10_u8.update_digest(digest);
                }
                Some(buf) => {
                    0b11_u8.update_digest(digest);
                    (buf.len() as u32).update_digest(digest);
                    digest.update(buf);
                }
            }
        }
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.finished_sub_slots.stream(out)?;
        self.reward_chain_block.stream(out)?;
        self.challenge_chain_sp_proof.stream(out)?;
        self.challenge_chain_ip_proof.stream(out)?;
        self.reward_chain_sp_proof.stream(out)?;
        self.reward_chain_ip_proof.stream(out)?;
        self.infused_challenge_chain_ip_proof.stream(out)?;
        self.foliage.stream(out)?;
        self.foliage_transaction_block.stream(out)?;
        self.transactions_info.stream(out)?;

        if self.version == 0 {
            self.transactions_generator.stream(out)?;
            self.transactions_generator_ref_list.stream(out)?;
        } else {
            match &self.transactions_generator_buffer {
                None => {
                    0b10_u8.stream(out)?;
                }
                Some(buf) => {
                    0b11_u8.stream(out)?;
                    (buf.len() as u32).stream(out)?;
                    out.extend_from_slice(buf);
                }
            }
        }
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let finished_sub_slots = <Vec<EndOfSubSlotBundle> as Streamable>::parse::<TRUSTED>(input)?;
        let reward_chain_block = <RewardChainBlock as Streamable>::parse::<TRUSTED>(input)?;
        let challenge_chain_sp_proof = <Option<VDFProof> as Streamable>::parse::<TRUSTED>(input)?;
        let challenge_chain_ip_proof = <VDFProof as Streamable>::parse::<TRUSTED>(input)?;
        let reward_chain_sp_proof = <Option<VDFProof> as Streamable>::parse::<TRUSTED>(input)?;
        let reward_chain_ip_proof = <VDFProof as Streamable>::parse::<TRUSTED>(input)?;
        let infused_challenge_chain_ip_proof =
            <Option<VDFProof> as Streamable>::parse::<TRUSTED>(input)?;
        let foliage = <Foliage as Streamable>::parse::<TRUSTED>(input)?;
        let foliage_transaction_block =
            <Option<FoliageTransactionBlock> as Streamable>::parse::<TRUSTED>(input)?;
        let transactions_info = <Option<TransactionsInfo> as Streamable>::parse::<TRUSTED>(input)?;

        let prefix = <u8 as Streamable>::parse::<TRUSTED>(input)?;
        let version = u8::from((prefix & 0b10) != 0);
        let has_generator = (prefix & 1) != 0;

        if version == 0 {
            let transactions_generator = if has_generator {
                Some(<Program as Streamable>::parse::<TRUSTED>(input)?)
            } else {
                None
            };
            let transactions_generator_ref_list =
                <Vec<u32> as Streamable>::parse::<TRUSTED>(input)?;

            Ok(FullBlock {
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
                transactions_generator,
                transactions_generator_ref_list,
                transactions_generator_buffer: None,
                version,
            })
        } else {
            let transactions_generator_buffer = if has_generator {
                let bytes = <Bytes as Streamable>::parse::<TRUSTED>(input)?;
                Some(bytes.into_inner())
            } else {
                None
            };

            Ok(FullBlock {
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
                transactions_generator: None,
                transactions_generator_ref_list: vec![],
                transactions_generator_buffer,
                version,
            })
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClassgroupElement, FoliageBlockData, PoolTarget, ProofOfSpace, VDFInfo};
    use chia_bls::{G1Element, G2Element};

    fn make_vdf_proof() -> VDFProof {
        VDFProof::new(0, Bytes::default(), false)
    }

    fn make_vdf_info() -> VDFInfo {
        VDFInfo::new(Bytes32::default(), 1, ClassgroupElement::default())
    }

    fn make_proof_of_space() -> ProofOfSpace {
        ProofOfSpace::new(
            Bytes32::default(),
            Some(G1Element::default()),
            None,
            G1Element::default(),
            0,
            0,
            0,
            0,
            32,
            Bytes::from(vec![0x80]),
        )
    }

    fn make_reward_chain_block() -> RewardChainBlock {
        RewardChainBlock::new(
            1,
            0,
            1,
            0,
            Bytes32::default(),
            make_proof_of_space(),
            None,
            G2Element::default(),
            make_vdf_info(),
            None,
            G2Element::default(),
            make_vdf_info(),
            None,
            None,
            false,
        )
    }

    fn make_foliage() -> Foliage {
        let pool_target = PoolTarget::new(Bytes32::default(), 0);
        let foliage_block_data = FoliageBlockData::new(
            Bytes32::default(),
            pool_target,
            Some(G2Element::default()),
            Bytes32::default(),
            Bytes32::default(),
        );
        Foliage::new(
            Bytes32::default(),
            Bytes32::default(),
            foliage_block_data,
            G2Element::default(),
            None,
            None,
        )
    }

    fn make_v0_block(generator: Option<Program>, ref_list: Vec<u32>) -> FullBlock {
        FullBlock::new(
            vec![],
            make_reward_chain_block(),
            None,
            make_vdf_proof(),
            None,
            make_vdf_proof(),
            None,
            make_foliage(),
            None,
            None,
            generator,
            ref_list,
            None,
            0,
        )
    }

    fn make_v1_block(buffer: Option<Vec<u8>>) -> FullBlock {
        FullBlock::new(
            vec![],
            make_reward_chain_block(),
            None,
            make_vdf_proof(),
            None,
            make_vdf_proof(),
            None,
            make_foliage(),
            None,
            None,
            None,
            vec![],
            buffer,
            1,
        )
    }

    #[test]
    fn v0_no_generator_roundtrip() {
        let block = make_v0_block(None, vec![]);
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::parse::<false>(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(block2.version, 0);
        assert!(block2.transactions_generator.is_none());
        assert!(block2.transactions_generator_ref_list.is_empty());
        assert!(block2.transactions_generator_buffer.is_none());
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v0_with_generator_roundtrip() {
        let generator = Program::from(vec![0xff, 0x01, 0x80]);
        let block = make_v0_block(Some(generator.clone()), vec![100, 200]);
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::parse::<false>(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(block2.version, 0);
        assert_eq!(
            block2.transactions_generator.as_ref().unwrap().as_ref(),
            generator.as_ref()
        );
        assert_eq!(block2.transactions_generator_ref_list, vec![100, 200]);
        assert!(block2.transactions_generator_buffer.is_none());
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v1_no_generator_roundtrip() {
        let block = make_v1_block(None);
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::parse::<false>(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(block2.version, 1);
        assert!(block2.transactions_generator.is_none());
        assert!(block2.transactions_generator_ref_list.is_empty());
        assert!(block2.transactions_generator_buffer.is_none());
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v1_with_buffer_roundtrip() {
        let raw = vec![0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe];
        let block = make_v1_block(Some(raw.clone()));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::parse::<false>(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(block2.version, 1);
        assert!(block2.transactions_generator.is_none());
        assert!(block2.transactions_generator_ref_list.is_empty());
        assert_eq!(block2.transactions_generator_buffer.as_ref().unwrap(), &raw);
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v0_prefix_byte_encoding() {
        let block_none = make_v0_block(None, vec![]);
        let buf_none = block_none.to_bytes().unwrap();

        let block_some = make_v0_block(Some(Program::from(vec![0x80])), vec![]);
        let buf_some = block_some.to_bytes().unwrap();

        let prefix_offset = buf_none
            .iter()
            .zip(buf_some.iter())
            .position(|(a, b)| a != b)
            .unwrap();

        assert_eq!(buf_none[prefix_offset], 0b00);
        assert_eq!(buf_some[prefix_offset], 0b01);
    }

    #[test]
    fn v1_prefix_byte_encoding() {
        let block_none = make_v1_block(None);
        let buf_none = block_none.to_bytes().unwrap();

        let block_some = make_v1_block(Some(vec![0x80]));
        let buf_some = block_some.to_bytes().unwrap();

        let prefix_offset = buf_none
            .iter()
            .zip(buf_some.iter())
            .position(|(a, b)| a != b)
            .unwrap();

        assert_eq!(buf_none[prefix_offset], 0b10);
        assert_eq!(buf_some[prefix_offset], 0b11);
    }

    #[test]
    fn v1_generator_has_length_prefix() {
        let raw = vec![0xca, 0xfe, 0xba, 0xbe];
        let block = make_v1_block(Some(raw.clone()));
        let buf = block.to_bytes().unwrap();

        let block_empty = make_v1_block(None);
        let buf_empty = block_empty.to_bytes().unwrap();

        let prefix_offset = buf
            .iter()
            .zip(buf_empty.iter())
            .position(|(a, b)| a != b)
            .unwrap();

        assert_eq!(buf[prefix_offset], 0b11);
        let len = u32::from_be_bytes(
            buf[prefix_offset + 1..prefix_offset + 5]
                .try_into()
                .unwrap(),
        );
        assert_eq!(len as usize, raw.len());
        assert_eq!(&buf[prefix_offset + 5..prefix_offset + 5 + raw.len()], &raw);
        assert_eq!(prefix_offset + 5 + raw.len(), buf.len());
    }

    #[test]
    fn v1_omits_ref_list() {
        let block_v0 = make_v0_block(Some(Program::from(vec![0x80])), vec![42]);
        let buf_v0 = block_v0.to_bytes().unwrap();

        let block_v1 = make_v1_block(Some(vec![0x80]));
        let buf_v1 = block_v1.to_bytes().unwrap();

        // v0: 1 (prefix) + 1 (program "80") + 4 (ref_list count) + 4 (one u32) = 10 tail bytes
        // v1: 1 (prefix) + 4 (length) + 1 (data) = 6 tail bytes
        assert!(buf_v1.len() < buf_v0.len());
    }

    #[test]
    fn v0_and_v1_same_hash_fields_before_generator() {
        let block_v0 = make_v0_block(None, vec![]);
        let block_v1 = make_v1_block(None);

        assert_eq!(block_v0.header_hash(), block_v1.header_hash());
    }

    #[test]
    fn v1_unvalidated_buffer_roundtrips() {
        let garbage = vec![0xff; 1000];
        let block = make_v1_block(Some(garbage.clone()));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::parse::<false>(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(block2.transactions_generator_buffer.unwrap(), garbage);
    }
}
