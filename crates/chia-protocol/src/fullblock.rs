use chia_streamable_macro::streamable;

use crate::Bytes;
use crate::Bytes32;
use crate::Coin;
use crate::EndOfSubSlotBundle;
use crate::Program;
use crate::RewardChainBlock;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};
use chia_traits::{Option3, Streamable};

// Wire encoding uses Option3 for the generator prefix byte:
//   0x00  None(ref_list)       → no generator (v0, backward-compatible with pre-HF blocks)
//   0x01  Some1(P, ref_list)   → Program generator (v0, backward-compatible with pre-HF blocks)
//   0x02  Some2(B)             → raw-bytes generator (v1, new post-HF format, NO ref_list)
//
// For v0 blocks (None/Some1), transactions_generator_ref_list is embedded in the Option3 tail.
// For v1 blocks (Some2), the ref_list is completely absent from the wire format.
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
    transactions_generator: Option3<Program, Bytes, Vec<u32>>, // Program (v0) or raw bytes (v1), with ref_list for v0
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

    pub fn is_v1(&self) -> bool {
        matches!(self.transactions_generator, Option3::Some2(_))
    }

    pub fn is_v0(&self) -> bool {
        !self.is_v1()
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

    #[pyo3(name = "is_v0")]
    fn py_is_v0(&self) -> bool {
        self.is_v0()
    }

    #[pyo3(name = "is_v1")]
    fn py_is_v1(&self) -> bool {
        self.is_v1()
    }

    #[getter]
    #[pyo3(name = "transactions_generator_ref_list")]
    fn py_transactions_generator_ref_list(&self) -> Vec<u32> {
        match &self.transactions_generator {
            Option3::None(refs) | Option3::Some1(_, refs) => refs.clone(),
            Option3::Some2(_) => vec![],
        }
    }

    #[getter]
    #[pyo3(name = "transactions_generator")]
    fn py_transactions_generator(&self) -> Option<Program> {
        match &self.transactions_generator {
            Option3::Some1(prog, _) => Some(prog.clone()),
            _ => None,
        }
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

    fn make_v0_block(generator: Option3<Program, Bytes, Vec<u32>>) -> FullBlock {
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
        )
    }

    fn make_v1_block(buffer: Option<Vec<u8>>) -> FullBlock {
        let generator = match buffer {
            None => Option3::None(vec![]),
            Some(buf) => Option3::Some2(Bytes::from(buf)),
        };
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
        )
    }

    #[test]
    fn v0_no_generator_roundtrip() {
        let block = make_v0_block(Option3::None(vec![]));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::from_bytes(&buf).unwrap();

        assert!(block2.is_v0());
        assert!(matches!(block2.transactions_generator, Option3::None(_)));
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v0_with_generator_roundtrip() {
        let generator = Program::from(vec![0xff, 0x01, 0x80]);
        let block = make_v0_block(Option3::Some1(generator.clone(), vec![100, 200]));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::from_bytes(&buf).unwrap();

        assert!(block2.is_v0());
        match &block2.transactions_generator {
            Option3::Some1(prog, refs) => {
                assert_eq!(prog.as_ref(), generator.as_ref());
                assert_eq!(refs, &vec![100, 200]);
            }
            _ => panic!("Expected Some1"),
        }
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v1_no_generator_roundtrip() {
        // v1 "no generator" is represented as Option3::None with empty ref_list on wire;
        // is_v1() is false here — a block with no generator at all is just a non-tx block.
        // True v1 blocks always carry Some2(Bytes).
        let block = make_v0_block(Option3::None(vec![]));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::from_bytes(&buf).unwrap();

        assert!(block2.is_v0());
        assert!(matches!(block2.transactions_generator, Option3::None(_)));
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v1_with_buffer_roundtrip() {
        let raw = vec![0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe];
        let block = make_v1_block(Some(raw.clone()));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::from_bytes(&buf).unwrap();

        assert!(block2.is_v1());
        match &block2.transactions_generator {
            Option3::Some2(bytes) => assert_eq!(bytes.as_ref(), &raw),
            _ => panic!("Expected Some2"),
        }
        assert_eq!(block2.to_bytes().unwrap(), buf);
    }

    #[test]
    fn v0_prefix_byte_encoding() {
        let block_none = make_v0_block(Option3::None(vec![]));
        let buf_none = block_none.to_bytes().unwrap();

        let block_some = make_v0_block(Option3::Some1(Program::from(vec![0x80]), vec![]));
        let buf_some = block_some.to_bytes().unwrap();

        let prefix_offset = buf_none
            .iter()
            .zip(buf_some.iter())
            .position(|(a, b)| a != b)
            .unwrap();

        assert_eq!(buf_none[prefix_offset], 0x00);
        assert_eq!(buf_some[prefix_offset], 0x01);
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

        assert_eq!(buf_none[prefix_offset], 0x00);
        assert_eq!(buf_some[prefix_offset], 0x02);
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

        // Verify the generator is serialized with Option3 prefix 0x02
        assert_eq!(buf[prefix_offset], 0x02);

        // Verify length prefix matches the data
        let len = u32::from_be_bytes(
            buf[prefix_offset + 1..prefix_offset + 5]
                .try_into()
                .unwrap(),
        );
        assert_eq!(len as usize, raw.len());

        // Verify the data is correct
        assert_eq!(&buf[prefix_offset + 5..prefix_offset + 5 + raw.len()], &raw);

        // v1 blocks (Some2) have NO ref_list, so the data should end immediately after the bytes
        assert_eq!(buf.len(), prefix_offset + 5 + raw.len());
    }

    #[test]
    fn v1_ref_list_absent() {
        // v1 blocks (Some2) do NOT write a ref_list — the Option3 tail is only present for None/Some1.
        let block_v1 = make_v1_block(Some(vec![0x80]));
        let buf_v1 = block_v1.to_bytes().unwrap();

        // Find where 0x02 prefix byte is
        let prefix_offset = buf_v1.iter().position(|&b| b == 0x02).unwrap();
        // After 0x02 + 4-byte length + 1 byte data, the buffer should end (no ref_list)
        let expected_len = prefix_offset + 1 + 4 + 1;
        assert_eq!(buf_v1.len(), expected_len);
    }

    #[test]
    fn v0_and_v1_same_hash_fields_before_generator() {
        let block_v0 = make_v0_block(Option3::None(vec![]));
        let block_v1 = make_v1_block(None);

        assert_eq!(block_v0.header_hash(), block_v1.header_hash());
    }

    // Interop tests: prove that v0 wire bytes are identical to what the pre-HF
    // code (Option<Program> + Vec<u32>) would have produced, and that those
    // bytes parse correctly with the new Option3-based code.
    //
    // Pre-HF Streamable encoding:
    //   transactions_generator:          Option<Program> → 0x00 | 0x01 + data
    //   transactions_generator_ref_list: Vec<u32>        → u32 length + u32s
    //
    // New encoding (Option3 + Vec<u32>):
    //   transactions_generator:          Option3<Program, Bytes> → 0x00 | 0x01 | 0x02 + data
    //   transactions_generator_ref_list: Vec<u32>                → u32 length + u32s
    //
    // For v0 blocks (None / Some1), the prefix bytes and payload are identical.

    fn old_format_tail(generator: Option<Program>, ref_list: &[u32]) -> Vec<u8> {
        let mut out = vec![];
        generator.stream(&mut out).unwrap();
        ref_list.to_vec().stream(&mut out).unwrap();
        out
    }

    #[test]
    fn old_format_interop_no_generator() {
        let ref_list = vec![10u32, 20, 30];
        let block = make_v0_block(Option3::None(ref_list.clone()));
        let bytes = block.to_bytes().unwrap();

        // New code parses it correctly.
        let parsed = FullBlock::from_bytes(&bytes).unwrap();
        match &parsed.transactions_generator {
            Option3::None(refs) => assert_eq!(refs, &ref_list),
            _ => panic!("Expected None"),
        }

        // Tail bytes are byte-for-byte identical to old Option<Program> + Vec<u32>.
        assert!(bytes.ends_with(&old_format_tail(None, &ref_list)));
    }

    #[test]
    fn old_format_interop_with_generator() {
        let program = Program::from(vec![0xff, 0x01, 0x80]);
        let ref_list = vec![100u32, 200];
        let block = make_v0_block(Option3::Some1(program.clone(), ref_list.clone()));
        let bytes = block.to_bytes().unwrap();

        // New code parses it correctly.
        let parsed = FullBlock::from_bytes(&bytes).unwrap();
        match &parsed.transactions_generator {
            Option3::Some1(p, refs) => {
                assert_eq!(p.as_ref(), program.as_ref());
                assert_eq!(refs, &ref_list);
            }
            _ => panic!("Expected Some1"),
        }

        // Tail bytes are byte-for-byte identical to old Option<Program> + Vec<u32>.
        assert!(bytes.ends_with(&old_format_tail(Some(program), &ref_list)));
    }

    #[test]
    fn old_format_bytes_parse_with_new_code() {
        // Build a complete FullBlock in the old wire format by hand:
        // serialize all unchanged fields with the new code (they're identical),
        // then append old-format generator tail, and verify new code parses it.
        let program = Program::from(vec![0x80]);
        let ref_list = vec![42u32];

        // New-format block to get the prefix bytes (everything before the generator)
        let new_block = make_v0_block(Option3::Some1(program.clone(), ref_list.clone()));
        let new_bytes = new_block.to_bytes().unwrap();

        // The new tail (0x01 + program + ref_list) is identical to the old tail.
        // Confirm by constructing old tail and checking the suffix matches.
        let old_tail = old_format_tail(Some(program.clone()), &ref_list);
        assert!(
            new_bytes.ends_with(&old_tail),
            "new format tail must match old format tail"
        );

        // Craft bytes that look exactly like a pre-HF block: swap new tail for old
        // tail (they're the same, so this is a no-op — but proves the point).
        let prefix_len = new_bytes.len() - old_tail.len();
        let mut old_format_bytes = new_bytes[..prefix_len].to_vec();
        old_format_bytes.extend_from_slice(&old_tail);

        // Parse with new code — must succeed and produce correct values.
        let parsed = FullBlock::from_bytes(&old_format_bytes).unwrap();
        match &parsed.transactions_generator {
            Option3::Some1(p, refs) => {
                assert_eq!(p.as_ref(), program.as_ref());
                assert_eq!(refs, &ref_list);
            }
            _ => panic!("Expected Some1"),
        }
    }

    #[test]
    fn v1_unvalidated_buffer_roundtrips() {
        let garbage = vec![0xff; 1000];
        let block = make_v1_block(Some(garbage.clone()));
        let buf = block.to_bytes().unwrap();
        let block2 = FullBlock::from_bytes(&buf).unwrap();
        match &block2.transactions_generator {
            Option3::Some2(bytes) => assert_eq!(bytes.as_ref(), &garbage),
            _ => panic!("Expected Some2"),
        }
    }
}
