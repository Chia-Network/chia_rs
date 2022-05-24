use core::fmt::Formatter;
use serde::ser::SerializeTuple;
use serde::{Deserialize, Serialize};
use std::convert::AsRef;
use std::fmt;
use std::fmt::Debug;
use std::ops::Deref;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyBytes;
#[cfg(feature = "py-bindings")]
use std::convert::TryInto;

#[derive(Serialize, Deserialize, Hash, Debug, Clone, Eq, PartialEq)]
pub struct Bytes(Vec<u8>);

impl From<&[u8]> for Bytes {
    fn from(v: &[u8]) -> Bytes {
        Bytes(v.to_vec())
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Bytes {
        Bytes(v)
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(&self.0))
    }
}

struct ByteVisitor<const N: usize>;

impl<'de, const N: usize> serde::de::Visitor<'de> for ByteVisitor<N> {
    type Value = BytesImpl<N>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: serde::de::SeqAccess<'de>,
    {
        let mut dest: [u8; N] = [0; N];
        let mut counter = 0;
        while let Some(value) = seq.next_element()? {
            dest[counter] = value;
            counter += 1;
        }

        Ok(dest.into())
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub struct BytesImpl<const N: usize>([u8; N]);

impl<const N: usize> Serialize for BytesImpl<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_tuple(N)?;
        for elem in &self.0[..] {
            seq.serialize_element(elem)?;
        }
        seq.end()
    }
}

impl<const N: usize> From<[u8; N]> for BytesImpl<N> {
    fn from(v: [u8; N]) -> BytesImpl<N> {
        BytesImpl::<N>(v)
    }
}
impl<const N: usize> From<&[u8; N]> for BytesImpl<N> {
    fn from(v: &[u8; N]) -> BytesImpl<N> {
        BytesImpl::<N>(*v)
    }
}
impl<const N: usize> From<&[u8]> for BytesImpl<N> {
    fn from(v: &[u8]) -> BytesImpl<N> {
        if v.len() != N {
            panic!("invalid atom, expected {} bytes (got {})", N, v.len());
        }
        let mut ret = BytesImpl::<N>([0; N]);
        ret.0.copy_from_slice(v);
        ret
    }
}
impl<const N: usize> From<Vec<u8>> for BytesImpl<N> {
    fn from(v: Vec<u8>) -> BytesImpl<N> {
        if v.len() != N {
            panic!("invalid atom, expected {} bytes (got {})", N, v.len());
        }
        let mut ret = BytesImpl::<N>([0; N]);
        ret.0.copy_from_slice(&v);
        ret
    }
}
impl<'a, const N: usize> From<&'a BytesImpl<N>> for &'a [u8; N] {
    fn from(v: &'a BytesImpl<N>) -> &'a [u8; N] {
        &v.0
    }
}

impl<'a, const N: usize> From<&'a BytesImpl<N>> for &'a [u8] {
    fn from(v: &'a BytesImpl<N>) -> &'a [u8] {
        &v.0
    }
}

impl<const N: usize> BytesImpl<N> {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl<const N: usize> AsRef<[u8]> for BytesImpl<N> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl<const N: usize> Deref for BytesImpl<N> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl<const N: usize> Debug for BytesImpl<N> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        formatter.write_str(&hex::encode(self.0))
    }
}
impl<const N: usize> fmt::Display for BytesImpl<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

impl<'de, const N: usize> Deserialize<'de> for BytesImpl<N> {
    fn deserialize<D>(deserializer: D) -> Result<BytesImpl<N>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_tuple(N, ByteVisitor::<N>)
    }
}

pub type Bytes32 = BytesImpl<32>;
pub type Bytes48 = BytesImpl<48>;
pub type Bytes96 = BytesImpl<96>;
pub type Bytes100 = BytesImpl<100>;

#[cfg(feature = "py-bindings")]
impl<const N: usize> ToPyObject for BytesImpl<N> {
    fn to_object(&self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

#[cfg(feature = "py-bindings")]
impl<const N: usize> IntoPy<PyObject> for BytesImpl<N> {
    fn into_py(self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

#[cfg(feature = "py-bindings")]
impl<'py, const N: usize> FromPyObject<'py> for BytesImpl<N> {
    fn extract(obj: &'py PyAny) -> PyResult<Self> {
        let b = <PyBytes as PyTryFrom>::try_from(obj)?;
        let slice: &[u8] = b.as_bytes();
        let buf: [u8; N] = slice.try_into()?;
        Ok(BytesImpl::<N>(buf))
    }
}

#[cfg(feature = "py-bindings")]
impl ToPyObject for Bytes {
    fn to_object(&self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

#[cfg(feature = "py-bindings")]
impl IntoPy<PyObject> for Bytes {
    fn into_py(self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

#[cfg(feature = "py-bindings")]
impl<'py> FromPyObject<'py> for Bytes {
    fn extract(obj: &'py PyAny) -> PyResult<Self> {
        let b = <PyBytes as PyTryFrom>::try_from(obj)?;
        Ok(Bytes(b.as_bytes().to_vec()))
    }
}
