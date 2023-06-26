use crate::chia_error;
use crate::streamable::{read_bytes, Streamable};
use core::fmt::Formatter;
use sha2::{Digest, Sha256};
use std::convert::AsRef;
use std::convert::TryInto;
use std::fmt;
use std::fmt::Debug;
use std::io::Cursor;
use std::ops::Deref;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyBytes;

#[derive(Hash, Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Streamable for Bytes {
    fn update_digest(&self, digest: &mut Sha256) {
        (self.0.len() as u32).update_digest(digest);
        digest.update(&self.0);
    }
    fn stream(&self, out: &mut Vec<u8>) -> chia_error::Result<()> {
        if self.0.len() > u32::MAX as usize {
            Err(chia_error::Error::SequenceTooLarge)
        } else {
            (self.0.len() as u32).stream(out)?;
            out.extend_from_slice(&self.0);
            Ok(())
        }
    }

    fn parse(input: &mut Cursor<&[u8]>) -> chia_error::Result<Self> {
        let len = u32::parse(input)?;
        Ok(Bytes(read_bytes(input, len as usize)?.to_vec()))
    }
}

impl PartialEq<Bytes> for Vec<u8> {
    fn eq(&self, lhs: &Bytes) -> bool {
        *self == lhs.0
    }
}

impl PartialEq<Vec<u8>> for Bytes {
    fn eq(&self, lhs: &Vec<u8>) -> bool {
        self.0 == *lhs
    }
}

impl PartialEq<&[u8]> for Bytes {
    fn eq(&self, lhs: &&[u8]) -> bool {
        &self.0 == lhs
    }
}

impl PartialEq<Bytes> for &[u8] {
    fn eq(&self, lhs: &Bytes) -> bool {
        self == &lhs.0
    }
}

impl<const N: usize> PartialEq<[u8; N]> for Bytes {
    fn eq(&self, lhs: &[u8; N]) -> bool {
        self.0 == lhs
    }
}

impl<const N: usize> PartialEq<&[u8; N]> for Bytes {
    fn eq(&self, lhs: &&[u8; N]) -> bool {
        self.0 == *lhs
    }
}

impl<const N: usize> PartialEq<Bytes> for &[u8; N] {
    fn eq(&self, lhs: &Bytes) -> bool {
        lhs.0 == *self
    }
}

impl<const N: usize> PartialEq<Bytes> for [u8; N] {
    fn eq(&self, lhs: &Bytes) -> bool {
        lhs.0 == self
    }
}

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

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(&self.0))
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, PartialOrd, Ord)]
pub struct BytesImpl<const N: usize>([u8; N]);

impl<const N: usize> Streamable for BytesImpl<N> {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.0);
    }
    fn stream(&self, out: &mut Vec<u8>) -> chia_error::Result<()> {
        out.extend_from_slice(&self.0);
        Ok(())
    }

    fn parse(input: &mut Cursor<&[u8]>) -> chia_error::Result<Self> {
        Ok(BytesImpl(read_bytes(input, N)?.try_into().unwrap()))
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
impl<const N: usize> From<&Vec<u8>> for BytesImpl<N> {
    fn from(v: &Vec<u8>) -> BytesImpl<N> {
        if v.len() != N {
            panic!("invalid atom, expected {} bytes (got {})", N, v.len());
        }
        let mut ret = BytesImpl::<N>([0; N]);
        ret.0.copy_from_slice(v);
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        formatter.write_str(&hex::encode(self.0))
    }
}
impl<const N: usize> fmt::Display for BytesImpl<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

impl<const N: usize> PartialEq<&[u8]> for BytesImpl<N> {
    fn eq(&self, lhs: &&[u8]) -> bool {
        self.0 == *lhs
    }
}

impl<const N: usize> PartialEq<BytesImpl<N>> for &[u8] {
    fn eq(&self, lhs: &BytesImpl<N>) -> bool {
        self == &lhs.0
    }
}

impl<const N: usize> PartialEq<&[u8; N]> for BytesImpl<N> {
    fn eq(&self, lhs: &&[u8; N]) -> bool {
        &self.0 == *lhs
    }
}

impl<const N: usize> PartialEq<BytesImpl<N>> for &[u8; N] {
    fn eq(&self, lhs: &BytesImpl<N>) -> bool {
        *self == &lhs.0
    }
}

impl<const N: usize> PartialEq<[u8; N]> for BytesImpl<N> {
    fn eq(&self, lhs: &[u8; N]) -> bool {
        &self.0 == lhs
    }
}

impl<const N: usize> PartialEq<BytesImpl<N>> for [u8; N] {
    fn eq(&self, lhs: &BytesImpl<N>) -> bool {
        self == &lhs.0
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

#[cfg(test)]
use rstest::rstest;

#[cfg(test)]
#[rstest]
// Bytess32
#[case(
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    true
)]
#[case(
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000100",
    false
)]
#[case(
    "fff0000000000000000000000000000000000000000000000000000000000100",
    "fff0000000000000000000000000000000000000000000000000000000000100",
    true
)]
// Bytes
#[case("000000", "000000", true)]
#[case("123456", "125456", false)]
#[case("000001", "00000001", false)]
#[case("00000001", "000001", false)]
#[case("ffff01", "ffff01", true)]
#[case("", "", true)]
fn test_bytes_comparisons(#[case] lhs: &str, #[case] rhs: &str, #[case] expect_equal: bool) {
    let lhs_vec: Vec<u8> = hex::decode(lhs).expect("hex::decode");
    let rhs_vec: Vec<u8> = hex::decode(rhs).expect("hex::decode");
    let lhs_slice = &lhs_vec[..];
    let rhs_slice = &rhs_vec[..];
    if lhs_vec.len() == 32 && rhs_vec.len() == 32 {
        let lhs = Bytes32::from(&lhs_vec);
        let rhs = Bytes32::from(&rhs_vec);

        assert_eq!(lhs.len(), 32);
        assert_eq!(rhs.len(), 32);

        assert_eq!(lhs.is_empty(), lhs_vec.is_empty());
        assert_eq!(rhs.is_empty(), rhs_vec.is_empty());

        // Bytes32 compare against arrays of the same size, not slices
        let lhs_array: &[u8; 32] = lhs_slice.try_into().unwrap();
        let rhs_array: &[u8; 32] = rhs_slice.try_into().unwrap();
        if expect_equal {
            assert_eq!(lhs, rhs);
            assert_eq!(rhs, lhs);

            // array comparisons
            assert_eq!(lhs, rhs_array);
            assert_eq!(rhs, lhs_array);
            assert_eq!(lhs_array, rhs);
            assert_eq!(rhs_array, lhs);

            assert_eq!(lhs, *rhs_array);
            assert_eq!(rhs, *lhs_array);
            assert_eq!(*lhs_array, rhs);
            assert_eq!(*rhs_array, lhs);

            // slice comparisona
            assert_eq!(lhs, rhs_slice);
            assert_eq!(rhs, lhs_slice);
            assert_eq!(lhs_slice, rhs);
            assert_eq!(rhs_slice, lhs);
        } else {
            assert!(lhs != rhs);
            assert!(rhs != lhs);

            // array comparisons
            assert!(lhs != rhs_array);
            assert!(rhs != lhs_array);
            assert!(lhs_array != rhs);
            assert!(rhs_array != lhs);

            assert!(lhs != *rhs_array);
            assert!(rhs != *lhs_array);
            assert!(*lhs_array != rhs);
            assert!(*rhs_array != lhs);

            // slice comparisons
            assert!(lhs != rhs_slice);
            assert!(rhs != lhs_slice);
            assert!(lhs_slice != rhs);
            assert!(rhs_slice != lhs);
        }
    } else {
        let lhs = Bytes::from(lhs_vec.clone());
        let rhs = Bytes::from(rhs_vec.clone());

        assert_eq!(lhs.len(), lhs_vec.len());
        assert_eq!(rhs.len(), rhs_vec.len());

        assert_eq!(lhs.is_empty(), lhs_vec.is_empty());
        assert_eq!(rhs.is_empty(), rhs_vec.is_empty());

        // array comparisons
        let array: &[u8; 3] = &[1, 2, 3];
        assert!(lhs != array);
        assert!(rhs != array);
        assert!(array != lhs);
        assert!(array != rhs);
        assert!(lhs != *array);
        assert!(rhs != *array);
        assert!(*array != lhs);
        assert!(*array != rhs);

        if expect_equal {
            assert_eq!(lhs, rhs);
            assert_eq!(rhs, lhs);

            // slice comparisons
            assert_eq!(lhs, rhs_slice);
            assert_eq!(rhs, lhs_slice);
            assert_eq!(lhs_slice, rhs);
            assert_eq!(rhs_slice, lhs);

            // vec comparisons
            assert_eq!(lhs, rhs_vec);
            assert_eq!(rhs, lhs_vec);
            assert_eq!(lhs_vec, rhs);
            assert_eq!(rhs_vec, lhs);
        } else {
            assert!(lhs != rhs);
            assert!(rhs != lhs);

            // slice comparisons
            assert!(lhs != rhs_slice);
            assert!(rhs != lhs_slice);
            assert!(lhs_slice != rhs);
            assert!(rhs_slice != lhs);

            // vec comparisons
            assert!(lhs != rhs_vec);
            assert!(rhs != lhs_vec);
            assert!(lhs_vec != rhs);
            assert!(rhs_vec != lhs);
        }
    }
}
