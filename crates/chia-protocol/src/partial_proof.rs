use crate::Bytes32;
use chia_sha2::Sha256;
use chia_streamable_macro::streamable;

#[cfg(feature = "py-bindings")]
use pyo3::pymethods;

#[streamable]
pub struct PartialProof {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    fragments: [u64; 16],
}

impl PartialProof {
    pub fn get_string(&self, strength: u8) -> Bytes32 {
        let mut sha256 = Sha256::new();
        sha256.update(serialize_quality(&self.fragments, strength));
        sha256.finalize().into()
    }
}

const NUM_CHAIN_LINKS: usize = 16;

/// out must point to exactly 129 bytes
/// serializes the QualityProof into the form that will be hashed together with
/// the challenge to determine the quality of ths proof. The quality is used to
/// check if it passes the current difficulty. The format is:
/// 1 byte: plot strength
/// repeat 16 times:
///   8 bytes: little-endian proof fragment
fn serialize_quality(
    fragments: &[u64; NUM_CHAIN_LINKS],
    strength: u8,
) -> [u8; NUM_CHAIN_LINKS * 8 + 1] {
    let mut ret = [0_u8; 129];

    ret[0] = strength;
    let mut idx = 1;
    for cl in fragments {
        ret[idx..(idx + 8)].clone_from_slice(&cl.to_le_bytes());
        idx += 8;
    }
    ret
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl PartialProof {
    #[pyo3(name = "get_string")]
    fn py_get_string(&self, strength: u8) -> Bytes32 {
        self.get_string(strength)
    }
}
