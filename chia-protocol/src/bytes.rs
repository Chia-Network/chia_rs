use chia_traits::{chia_error, read_bytes, Streamable};
use clvm_traits::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};
use sha2::{Digest, Sha256};
use std::array::TryFromSliceError;
use std::convert::{AsRef, TryInto};
use std::fmt;
use std::io::Cursor;
use std::ops::Deref;

#[cfg(feature = "py-bindings")]
use chia_traits::{ChiaToPython, FromJsonDict, ToJsonDict};
#[cfg(feature = "py-bindings")]
use hex::FromHex;
#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyValueError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyBytes;

#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
pub struct Bytes(Vec<u8>);

impl Bytes {
    pub fn new(bytes: Vec<u8>) -> Self {
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

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self))
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self))
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

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> chia_error::Result<Self> {
        let len = u32::parse::<TRUSTED>(input)?;
        Ok(Bytes(read_bytes(input, len as usize)?.to_vec()))
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for Bytes {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        Ok(format!("0x{self}").to_object(py))
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for Bytes {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        let s: String = o.extract()?;
        if !s.starts_with("0x") {
            return Err(PyValueError::new_err(
                "bytes object is expected to start with 0x",
            ));
        }
        let s = &s[2..];
        let buf = match Vec::from_hex(s) {
            Err(_) => {
                return Err(PyValueError::new_err("invalid hex"));
            }
            Ok(v) => v,
        };
        Ok(buf.into())
    }
}

impl<N> ToClvm<N> for Bytes {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(self.0.as_slice())
    }
}

impl<N> FromClvm<N> for Bytes {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let bytes = decoder.decode_atom(&node)?;
        Ok(Self(bytes.to_vec()))
    }
}

impl From<&[u8]> for Bytes {
    fn from(value: &[u8]) -> Self {
        Self(value.to_vec())
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(value: Bytes) -> Self {
        value.0
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
pub struct BytesImpl<const N: usize>([u8; N]);

impl<const N: usize> BytesImpl<N> {
    pub fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn to_bytes(self) -> [u8; N] {
        self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl<const N: usize> Default for BytesImpl<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> fmt::Debug for BytesImpl<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str(&hex::encode(self))
    }
}

impl<const N: usize> fmt::Display for BytesImpl<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self))
    }
}

impl<const N: usize> Streamable for BytesImpl<N> {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.0);
    }
    fn stream(&self, out: &mut Vec<u8>) -> chia_error::Result<()> {
        out.extend_from_slice(&self.0);
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> chia_error::Result<Self> {
        Ok(BytesImpl(read_bytes(input, N)?.try_into().unwrap()))
    }
}

#[cfg(feature = "py-bindings")]
impl<const N: usize> ToJsonDict for BytesImpl<N> {
    fn to_json_dict(&self, py: Python) -> PyResult<PyObject> {
        Ok(format!("0x{self}").to_object(py))
    }
}

#[cfg(feature = "py-bindings")]
impl<const N: usize> FromJsonDict for BytesImpl<N> {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        let s: String = o.extract()?;
        if !s.starts_with("0x") {
            return Err(PyValueError::new_err(
                "bytes object is expected to start with 0x",
            ));
        }
        let s = &s[2..];
        let buf = match Vec::from_hex(s) {
            Err(_) => {
                return Err(PyValueError::new_err("invalid hex"));
            }
            Ok(v) => v,
        };
        if buf.len() != N {
            return Err(PyValueError::new_err(format!(
                "invalid length {} expected {}",
                buf.len(),
                N
            )));
        }
        Ok(buf.try_into().unwrap())
    }
}

impl<N, const LEN: usize> ToClvm<N> for BytesImpl<LEN> {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(self.0.as_slice())
    }
}

impl<N, const LEN: usize> FromClvm<N> for BytesImpl<LEN> {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let bytes = decoder.decode_atom(&node)?;
        if bytes.len() != LEN {
            return Err(FromClvmError::WrongAtomLength {
                expected: LEN,
                found: bytes.len(),
            });
        }
        Ok(Self::try_from(bytes).unwrap())
    }
}

impl<const N: usize> TryFrom<&[u8]> for BytesImpl<N> {
    type Error = TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, TryFromSliceError> {
        Ok(Self(value.try_into()?))
    }
}

impl<const N: usize> TryFrom<Vec<u8>> for BytesImpl<N> {
    type Error = TryFromSliceError;

    fn try_from(value: Vec<u8>) -> Result<Self, TryFromSliceError> {
        value.as_slice().try_into()
    }
}

impl<const N: usize> TryFrom<&Vec<u8>> for BytesImpl<N> {
    type Error = TryFromSliceError;

    fn try_from(value: &Vec<u8>) -> Result<Self, TryFromSliceError> {
        value.as_slice().try_into()
    }
}

impl<const N: usize> From<BytesImpl<N>> for Vec<u8> {
    fn from(value: BytesImpl<N>) -> Self {
        value.to_vec()
    }
}

impl<const N: usize> From<[u8; N]> for BytesImpl<N> {
    fn from(value: [u8; N]) -> Self {
        Self(value)
    }
}

impl<const N: usize> From<&[u8; N]> for BytesImpl<N> {
    fn from(value: &[u8; N]) -> Self {
        Self(*value)
    }
}

impl<const N: usize> From<BytesImpl<N>> for [u8; N] {
    fn from(value: BytesImpl<N>) -> Self {
        value.0
    }
}

impl<'a, const N: usize> From<&'a BytesImpl<N>> for &'a [u8; N] {
    fn from(value: &'a BytesImpl<N>) -> &'a [u8; N] {
        &value.0
    }
}

impl<const N: usize> From<&BytesImpl<N>> for [u8; N] {
    fn from(value: &BytesImpl<N>) -> [u8; N] {
        value.0
    }
}

impl<'a, const N: usize> From<&'a BytesImpl<N>> for &'a [u8] {
    fn from(value: &'a BytesImpl<N>) -> &'a [u8] {
        &value.0
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
impl<const N: usize> ChiaToPython for BytesImpl<N> {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        Ok(PyBytes::new(py, &self.0).into())
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
impl ChiaToPython for Bytes {
    fn to_python<'a>(&self, py: Python<'a>) -> PyResult<&'a PyAny> {
        Ok(PyBytes::new(py, &self.0).into())
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
mod tests {
    use super::*;

    use clvmr::{
        serde::{node_from_bytes, node_to_bytes},
        Allocator,
    };
    use rstest::rstest;

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

        if lhs_vec.len() == 32 && rhs_vec.len() == 32 {
            let lhs = Bytes32::try_from(&lhs_vec).unwrap();
            let rhs = Bytes32::try_from(&rhs_vec).unwrap();

            assert_eq!(lhs.len(), 32);
            assert_eq!(rhs.len(), 32);

            assert_eq!(lhs.is_empty(), lhs_vec.is_empty());
            assert_eq!(rhs.is_empty(), rhs_vec.is_empty());

            if expect_equal {
                assert_eq!(lhs, rhs);
                assert_eq!(rhs, lhs);
            } else {
                assert!(lhs != rhs);
                assert!(rhs != lhs);
            }
        } else {
            let lhs = Bytes::from(lhs_vec.clone());
            let rhs = Bytes::from(rhs_vec.clone());

            assert_eq!(lhs.len(), lhs_vec.len());
            assert_eq!(rhs.len(), rhs_vec.len());

            assert_eq!(lhs.is_empty(), lhs_vec.is_empty());
            assert_eq!(rhs.is_empty(), rhs_vec.is_empty());

            if expect_equal {
                assert_eq!(lhs, rhs);
                assert_eq!(rhs, lhs);
            } else {
                assert!(lhs != rhs);
                assert!(rhs != lhs);
            }
        }
    }

    fn from_bytes<T: Streamable + std::fmt::Debug + std::cmp::PartialEq>(buf: &[u8], expected: T) {
        let mut input = Cursor::<&[u8]>::new(buf);
        assert_eq!(T::parse::<false>(&mut input).unwrap(), expected);
    }

    fn from_bytes_fail<T: Streamable + std::fmt::Debug + std::cmp::PartialEq>(
        buf: &[u8],
        expected: chia_error::Error,
    ) {
        let mut input = Cursor::<&[u8]>::new(buf);
        assert_eq!(T::parse::<false>(&mut input).unwrap_err(), expected);
    }

    fn stream<T: Streamable>(v: &T) -> Vec<u8> {
        let mut buf = Vec::<u8>::new();
        v.stream(&mut buf).unwrap();
        let mut ctx1 = Sha256::new();
        let mut ctx2 = Sha256::new();
        v.update_digest(&mut ctx1);
        ctx2.update(&buf);
        assert_eq!(&ctx1.finalize(), &ctx2.finalize());
        buf
    }

    #[test]
    fn test_stream_bytes32() {
        let buf = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let out = stream(&Bytes32::from(buf));
        assert_eq!(buf.as_slice(), &out);
    }

    #[test]
    fn test_stream_bytes() {
        let val: Bytes = vec![
            1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32,
        ]
        .into();
        println!("{:?}", val);
        let buf = stream(&val);
        println!("buf: {:?}", buf);
        from_bytes(&buf, val);
    }

    #[test]
    fn test_parse_bytes_empty() {
        let buf: &[u8] = &[0, 0, 0, 0];
        from_bytes::<Bytes>(buf, [].to_vec().into());
    }

    #[test]
    fn test_parse_bytes() {
        let buf: &[u8] = &[0, 0, 0, 3, 1, 2, 3];
        from_bytes::<Bytes>(buf, [1_u8, 2, 3].to_vec().into());
    }

    #[test]
    fn test_parse_truncated_len() {
        let buf: &[u8] = &[0, 0, 1];
        from_bytes_fail::<Bytes>(buf, chia_error::Error::EndOfBuffer);
    }

    #[test]
    fn test_parse_truncated() {
        let buf: &[u8] = &[0, 0, 0, 4, 1, 2, 3];
        from_bytes_fail::<Bytes>(buf, chia_error::Error::EndOfBuffer);
    }

    #[test]
    fn test_parse_bytes32() {
        let buf = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        from_bytes::<Bytes32>(&buf, Bytes32::from(buf));
        from_bytes_fail::<Bytes32>(&buf[0..30], chia_error::Error::EndOfBuffer);
    }

    #[test]
    fn test_parse_bytes48() {
        let buf = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
            47, 48,
        ];
        from_bytes::<Bytes48>(&buf, Bytes48::from(buf));
        from_bytes_fail::<Bytes48>(&buf[0..47], chia_error::Error::EndOfBuffer);
    }

    #[test]
    fn bytes_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "84facef00d";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let bytes = Bytes::from_clvm(a, ptr).unwrap();

        let round_trip = bytes.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }

    #[test]
    fn bytes32_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "a0eff07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let bytes32 = Bytes32::from_clvm(a, ptr).unwrap();

        let round_trip = bytes32.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }

    #[test]
    fn bytes32_failure() {
        let a = &mut Allocator::new();
        let bytes =
            hex::decode("f07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9").unwrap();
        let ptr = a.new_atom(&bytes).unwrap();
        assert!(Bytes32::from_clvm(a, ptr).is_err());

        let ptr = a.new_pair(a.one(), a.one()).unwrap();
        assert_eq!(
            Bytes32::from_clvm(a, ptr).unwrap_err(),
            FromClvmError::ExpectedAtom
        );
    }
}
