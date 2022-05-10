use core::fmt::Formatter;
use std::convert::TryInto;
use std::fmt::Debug;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Bytes(Vec<u8>);

impl From<&[u8]> for Bytes {
    fn from(v: &[u8]) -> Bytes {
        Bytes(v.to_vec())
    }
}
#[derive(Hash, PartialEq, Eq, Copy, Clone)]
pub struct BytesImpl<const N: usize>([u8; N]);

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
    pub fn slice(&self) -> &[u8; N] {
        &self.0
    }
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl<const N: usize> Debug for BytesImpl<N> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        formatter.write_str(&hex::encode(self.0))
    }
}

pub type Bytes32 = BytesImpl<32>;
pub type Bytes48 = BytesImpl<48>;
pub type Bytes96 = BytesImpl<96>;
pub type Bytes100 = BytesImpl<100>;

impl<const N: usize> ToPyObject for BytesImpl<N> {
    fn to_object(&self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

impl<const N: usize> IntoPy<PyObject> for BytesImpl<N> {
    fn into_py(self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

impl<'py, const N: usize> FromPyObject<'py> for BytesImpl<N> {
    fn extract(obj: &'py PyAny) -> PyResult<Self> {
        let b = <PyBytes as PyTryFrom>::try_from(obj)?;
        let slice: &[u8] = b.as_bytes();
        let buf: [u8; N] = slice.try_into()?;
        Ok(BytesImpl::<N>(buf))
    }
}

impl ToPyObject for Bytes {
    fn to_object(&self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

impl IntoPy<PyObject> for Bytes {
    fn into_py(self, py: Python) -> PyObject {
        PyBytes::new(py, &self.0).into()
    }
}

impl<'py> FromPyObject<'py> for Bytes {
    fn extract(obj: &'py PyAny) -> PyResult<Self> {
        let b = <PyBytes as PyTryFrom>::try_from(obj)?;
        Ok(Bytes(b.as_bytes().to_vec()))
    }
}
