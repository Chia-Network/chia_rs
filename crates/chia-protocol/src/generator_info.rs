use crate::{Bytes, Program};
use chia_sha2::Sha256;
use chia_traits::chia_error::{Error, Result};
use chia_traits::Streamable;
use clvmr::serde::{serialized_length_from_bytes, serialized_length_serde_2026, SERDE_2026_MAGIC_PREFIX};
use std::io::Cursor;

/// Opaque blob containing both transactions_generator and transactions_generator_ref_list.
///
/// **Note:** This type is infrastructure for future FullBlock refactoring. It is not
/// currently used by FullBlock, which still stores generator and ref_list as separate
/// fields. A follow-up PR will migrate FullBlock to use this type.
///
/// This type defers parsing of generator data until validation time, when HF2 context
/// (block height) is available to determine which format to use.
///
/// Wire format (unchanged from current FullBlock format):
/// - Pre-HF2: [classic/serde_2026 generator bytes][ref_list: Vec<u32>]
/// - Post-HF2: [serde_2026 generator bytes][ref_list: Vec<u32>] (ref_list should be empty)
///
/// The blob is stored as raw bytes and parsed on-demand via `parse_generator_info()`.
///
/// # Migration Plan
///
/// 1. Land this type (this PR)
/// 2. Change FullBlock to use `generator_info: Option<GeneratorInfo>` (follow-up PR)
/// 3. Update validation code to call `parse_generator_info()` (follow-up PR)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeneratorInfo(Bytes);

impl GeneratorInfo {
    /// Create from raw bytes (used during deserialization)
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self(bytes)
    }

    /// Get raw bytes (used during serialization)
    pub fn as_bytes(&self) -> &Bytes {
        &self.0
    }

    /// Parse the blob into (generator, ref_list).
    ///
    /// Uses serialized_length_serde_2026() or serialized_length_from_bytes()
    /// to find the generator/ref_list boundary. Works for both pre-HF2 and
    /// post-HF2 blocks (post-HF2 ref_list should be empty, validated elsewhere).
    pub fn parse_generator_info(&self) -> Result<(Program, Vec<u32>)> {
        let blob = self.0.as_slice();
        
        // Find where the generator ends using the appropriate length function
        let gen_len = if blob.starts_with(&SERDE_2026_MAGIC_PREFIX) {
            serialized_length_serde_2026(blob, usize::MAX, false)
                .map_err(|_| Error::EndOfBuffer)?
        } else {
            serialized_length_from_bytes(blob)
                .map_err(|_| Error::EndOfBuffer)?
        };

        if blob.len() < gen_len as usize {
            return Err(Error::EndOfBuffer);
        }

        // Extract generator
        let generator = Program::from(&blob[..gen_len as usize]);

        // Parse ref_list from remaining bytes
        let mut cursor = Cursor::new(&blob[gen_len as usize..]);
        let ref_list = Vec::<u32>::parse::<false>(&mut cursor)?;

        Ok((generator, ref_list))
    }
}

impl Streamable for GeneratorInfo {
    fn update_digest(&self, digest: &mut Sha256) {
        self.0.update_digest(digest);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        // Just write the raw bytes (generator + ref_list in original wire format)
        out.extend_from_slice(self.0.as_slice());
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        // Read all remaining bytes to EOF
        let pos = input.position() as usize;
        let buf = input.get_ref();
        let remaining = &buf[pos..];
        
        input.set_position(buf.len() as u64);
        Ok(Self(Bytes::from(remaining)))
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::ToJsonDict;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
impl ToJsonDict for GeneratorInfo {
    fn to_json_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.0.to_json_dict(py)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clvmr::Allocator;
    use clvmr::serde::node_to_bytes;

    #[test]
    fn test_generator_info_roundtrip() {
        // Create a simple generator and ref_list
        let mut a = Allocator::new();
        let node = a.new_atom(b"test").unwrap();
        let gen_bytes = node_to_bytes(&a, node).unwrap();
        let generator = Program::from(&gen_bytes[..]);
        
        let ref_list = vec![100u32, 200u32];

        // Serialize to blob format
        let mut blob = Vec::new();
        blob.extend_from_slice(generator.as_slice());
        ref_list.stream(&mut blob).unwrap();

        // Create GeneratorInfo from blob
        let gen_info = GeneratorInfo::from_bytes(Bytes::from(blob.clone()));

        // Parse it back
        let (parsed_gen, parsed_ref_list) = gen_info.parse_generator_info().unwrap();

        assert_eq!(parsed_gen.as_slice(), generator.as_slice());
        assert_eq!(parsed_ref_list, ref_list);

        // Test streaming roundtrip
        let mut out = Vec::new();
        gen_info.stream(&mut out).unwrap();
        assert_eq!(out, blob);
    }
}
