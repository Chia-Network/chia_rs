use crate::bytes::Bytes;
use chia_traits::chia_error::{Error, Result};
use chia_traits::Streamable;
use clvmr::serde::serialized_length_from_bytes;
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pyclass, derive(PyStreamable))]
#[derive(Hash, Debug, Clone, Eq, PartialEq)]
pub struct Program(Bytes);

impl Program {
    pub fn new(bytes: Bytes) -> Self {
        Self(bytes)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
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

#[cfg(feature = "py-bindings")]
impl ToJsonDict for Program {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        self.0.to_json_dict(py)
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for Program {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        let bytes = Bytes::from_json_dict(o)?;
        let len =
            serialized_length_from_bytes(bytes.as_slice()).map_err(|_e| Error::EndOfBuffer)?;
        if len as usize != bytes.len() {
            // If the bytes in the JSON string is not a valid CLVM
            // serialization, or if it has garbage at the end of the string,
            // reject it
            return Err(Error::InvalidClvm)?;
        }
        Ok(Self(bytes))
    }
}

impl AsRef<[u8]> for Program {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}
