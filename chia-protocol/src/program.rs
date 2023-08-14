use crate::bytes::Bytes;
use chia_traits::chia_error::{Error, Result};
use chia_traits::Streamable;
use clvm_traits::{FromClvm, ToClvm};
use clvmr::allocator::NodePtr;
use clvmr::serde::{node_from_bytes, node_to_bytes, serialized_length_from_bytes};
use clvmr::Allocator;
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

#[cfg(feature = "py-bindings")]
impl ToJsonDict for Program {
    fn to_json_dict(&self, py: Python) -> pyo3::PyResult<PyObject> {
        self.0.to_json_dict(py)
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for Program {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        Ok(Self(Bytes::from_json_dict(o)?))
    }
}

impl FromClvm for Program {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> clvm_traits::Result<Self> {
        Ok(Self(
            node_to_bytes(a, ptr)
                .map_err(|error| clvm_traits::Error::Custom(error.to_string()))?
                .into(),
        ))
    }
}

impl ToClvm for Program {
    fn to_clvm(&self, a: &mut Allocator) -> clvm_traits::Result<NodePtr> {
        Ok(node_from_bytes(a, self.0.as_ref())
            .map_err(|error| clvm_traits::Error::Custom(error.to_string()))?)
    }
}

impl AsRef<[u8]> for Program {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "ff01ff02ff62ff0480";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let program = Program::from_clvm(a, ptr).unwrap();

        let round_trip = program.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }
}
