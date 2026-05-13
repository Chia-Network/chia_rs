use crate::{Bytes, Program};
use chia_sha2::Sha256;
use chia_traits::Streamable;
use chia_traits::chia_error::{Error, Result};
use std::io::Cursor;

/// Opaque blob containing both transactions_generator and transactions_generator_ref_list.
///
/// FullBlock uses this to defer parsing generator data until callers need it.
/// A future serde_2026-aware accessor can use block height/HF2 context to
/// decide which generator format to parse.
///
/// Wire format (unchanged from current FullBlock format):
/// - [transactions_generator: Option<Program>][transactions_generator_ref_list: Vec<u32>]
///
/// The blob is stored as raw bytes and parsed on-demand via `parse_generator_info()`.
///
/// The old eager `FullBlock` layout lives only in the proving tool that checks
/// this representation against mainnet blocks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeneratorInfo(Bytes);

impl GeneratorInfo {
    /// Create from raw bytes (used during deserialization)
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self(bytes)
    }

    /// Build from the public FullBlock generator fields.
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_parts(
        transactions_generator: Option<Program>,
        transactions_generator_ref_list: Vec<u32>,
    ) -> Self {
        let mut bytes = Vec::new();
        transactions_generator
            .stream(&mut bytes)
            .expect("streaming transactions_generator into memory cannot fail");
        transactions_generator_ref_list
            .stream(&mut bytes)
            .expect("streaming transactions_generator_ref_list into memory cannot fail");
        Self(Bytes::from(bytes))
    }

    /// Get raw bytes (used during serialization)
    pub fn as_bytes(&self) -> &Bytes {
        &self.0
    }

    /// Parse the blob into (generator, ref_list).
    ///
    /// Program parsing still self-frames to split the optional generator from
    /// the ref-list, but this work is deferred until the caller needs it.
    pub fn parse_generator_info(&self) -> Result<(Option<Program>, Vec<u32>)> {
        let blob = self.0.as_slice();
        let mut cursor = Cursor::new(blob);

        let generator = Option::<Program>::parse::<false>(&mut cursor)?;
        let ref_list = Vec::<u32>::parse::<false>(&mut cursor)?;
        if cursor.position() != blob.len() as u64 {
            return Err(Error::InputTooLarge);
        }

        Ok((generator, ref_list))
    }
}

impl Streamable for GeneratorInfo {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.0.as_slice());
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
    use chia_traits::chia_error::Error;
    use clvmr::Allocator;
    use clvmr::serde::node_to_bytes;

    fn test_program() -> Program {
        let mut a = Allocator::new();
        let node = a.new_atom(b"test").unwrap();
        let gen_bytes = node_to_bytes(&a, node).unwrap();
        Program::from(&gen_bytes[..])
    }

    fn generator_info_blob(generator: Option<&Program>, ref_list: &[u32]) -> Vec<u8> {
        let mut blob = Vec::new();
        match generator {
            Some(generator) => {
                blob.push(1);
                generator.stream(&mut blob).unwrap();
            }
            None => blob.push(0),
        }
        ref_list.to_vec().stream(&mut blob).unwrap();
        blob
    }

    #[test]
    fn test_generator_info_roundtrip() {
        // Create a simple generator and ref_list
        let generator = test_program();
        let ref_list = vec![100u32, 200u32];

        // Serialize to blob format
        let blob = generator_info_blob(Some(&generator), &ref_list);

        // Create GeneratorInfo from blob
        let gen_info = GeneratorInfo::from_bytes(Bytes::from(blob.clone()));

        // Parse it back
        let (parsed_gen, parsed_ref_list) = gen_info.parse_generator_info().unwrap();

        assert_eq!(parsed_gen.unwrap().as_slice(), generator.as_slice());
        assert_eq!(parsed_ref_list, ref_list);

        // Test streaming roundtrip
        let mut out = Vec::new();
        gen_info.stream(&mut out).unwrap();
        assert_eq!(out, blob);
    }

    #[test]
    fn test_generator_info_absent_generator_empty_ref_list() {
        let blob = generator_info_blob(None, &[]);
        let gen_info = GeneratorInfo::from_bytes(Bytes::from(blob.clone()));

        let (parsed_gen, parsed_ref_list) = gen_info.parse_generator_info().unwrap();

        assert!(parsed_gen.is_none());
        assert!(parsed_ref_list.is_empty());

        let mut out = Vec::new();
        gen_info.stream(&mut out).unwrap();
        assert_eq!(out, blob);
    }

    #[test]
    fn test_generator_info_present_generator_empty_ref_list() {
        let generator = test_program();
        let blob = generator_info_blob(Some(&generator), &[]);
        let gen_info = GeneratorInfo::from_bytes(Bytes::from(blob));

        let (parsed_gen, parsed_ref_list) = gen_info.parse_generator_info().unwrap();

        assert_eq!(parsed_gen.unwrap().as_slice(), generator.as_slice());
        assert!(parsed_ref_list.is_empty());
    }

    #[test]
    fn test_generator_info_rejects_trailing_bytes() {
        let generator = test_program();
        let mut blob = generator_info_blob(Some(&generator), &[42]);
        blob.push(0xff);
        let gen_info = GeneratorInfo::from_bytes(Bytes::from(blob));

        assert_eq!(
            gen_info.parse_generator_info().unwrap_err(),
            Error::InputTooLarge
        );
    }
}
