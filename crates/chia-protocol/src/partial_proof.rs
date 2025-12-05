use crate::Bytes32;
use chia_pos2::serialize_quality;
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

#[cfg(feature = "py-bindings")]
#[pymethods]
impl PartialProof {
    #[pyo3(name = "get_string")]
    fn py_get_string(&self, strength: u8) -> Bytes32 {
        self.get_string(strength)
    }
}
