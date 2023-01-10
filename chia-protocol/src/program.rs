use crate::bytes::Bytes;
use crate::chia_error::{Error, Result};
use crate::streamable::Streamable;
use clvmr::serde::serialized_length_from_bytes;
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[cfg(feature = "py-bindings")]
use crate::chia_error;
#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pyclass, derive(PyStreamable))]
#[derive(Hash, Debug, Clone, Eq, PartialEq)]
pub struct Program(Bytes);

impl Program {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Streamable for Program {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(&self.0);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(self.0.as_ref());
        Ok(())
    }

    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let pos = input.position();
        let buf: &[u8] = &input.get_ref()[pos as usize..];
        let len = serialized_length_from_bytes(buf).map_err(|_e| Error::EndOfBuffer)?;
        if buf.len() < len as usize {
            return Err(Error::EndOfBuffer);
        }
        let program = buf[..len as usize].to_vec();
        input.set_position(pos + len);
        Ok(Program(program.into()))
    }
}

impl AsRef<[u8]> for Program {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}
