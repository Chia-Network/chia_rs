use num_bigint::BigInt;

use crate::{ClvmEncoder, ToClvmError};

pub trait ToClvm<N> {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError>;
}

pub fn simplify_int_bytes(mut slice: &[u8]) -> &[u8] {
    while (!slice.is_empty()) && (slice[0] == 0) {
        if slice.len() > 1 && (slice[1] & 0x80 == 0x80) {
            break;
        }
        slice = &slice[1..];
    }
    slice
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl<N> ToClvm<N> for $primitive {
            fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
                let number = BigInt::from(*self);
                encoder.encode_atom(simplify_int_bytes(&number.to_signed_bytes_be()))
            }
        }
    };
}

clvm_primitive!(u8);
clvm_primitive!(i8);
clvm_primitive!(u16);
clvm_primitive!(i16);
clvm_primitive!(u32);
clvm_primitive!(i32);
clvm_primitive!(u64);
clvm_primitive!(i64);
clvm_primitive!(u128);
clvm_primitive!(i128);
clvm_primitive!(usize);
clvm_primitive!(isize);

impl<N, T> ToClvm<N> for &T
where
    T: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        T::to_clvm(*self, encoder)
    }
}

impl<N, A, B> ToClvm<N> for (A, B)
where
    A: ToClvm<N>,
    B: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        let first = self.0.to_clvm(encoder)?;
        let rest = self.1.to_clvm(encoder)?;
        encoder.encode_pair(first, rest)
    }
}

impl<N> ToClvm<N> for () {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(&[])
    }
}

impl<N, T> ToClvm<N> for &[T]
where
    T: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        let mut result = encoder.encode_atom(&[])?;
        for item in self.iter().rev() {
            let value = item.to_clvm(encoder)?;
            result = encoder.encode_pair(value, result)?;
        }
        Ok(result)
    }
}

impl<N, T, const LEN: usize> ToClvm<N> for [T; LEN]
where
    T: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        self.as_slice().to_clvm(encoder)
    }
}

impl<N, T> ToClvm<N> for Vec<T>
where
    T: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        self.as_slice().to_clvm(encoder)
    }
}

impl<N, T> ToClvm<N> for Option<T>
where
    T: ToClvm<N>,
{
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        match self {
            Some(value) => value.to_clvm(encoder),
            None => encoder.encode_atom(&[]),
        }
    }
}

impl<N> ToClvm<N> for &str {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(self.as_bytes())
    }
}

impl<N> ToClvm<N> for String {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        self.as_str().to_clvm(encoder)
    }
}

#[cfg(feature = "chia-bls")]
impl<N> ToClvm<N> for chia_bls::PublicKey {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(&self.to_bytes())
    }
}

#[cfg(feature = "chia-bls")]
impl<N> ToClvm<N> for chia_bls::Signature {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(&self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::{node_to_str, TestAllocator, TestNode};

    use super::*;

    fn encode<T>(value: T) -> Result<String, ToClvmError>
    where
        T: ToClvm<TestNode>,
    {
        let mut a = TestAllocator::new();
        let node = value.to_clvm(&mut a).unwrap();
        Ok(node_to_str(&a, &node))
    }

    #[test]
    fn test_primitives() {
        assert_eq!(encode(0u8), Ok("NIL".to_owned()));
        assert_eq!(encode(0i8), Ok("NIL".to_owned()));
        assert_eq!(encode(5u8), Ok("05".to_owned()));
        assert_eq!(encode(5u32), Ok("05".to_owned()));
        assert_eq!(encode(5i32), Ok("05".to_owned()));
        assert_eq!(encode(-27i32), Ok("e5".to_owned()));
        assert_eq!(encode(-0), Ok("NIL".to_owned()));
        assert_eq!(encode(-128i8), Ok("80".to_owned()));
    }

    #[test]
    fn test_reference() {
        assert_eq!(encode([1, 2, 3]), encode([1, 2, 3]));
        assert_eq!(encode(Some(42)), encode(Some(42)));
        assert_eq!(encode(Some(&42)), encode(Some(42)));
        assert_eq!(encode(Some(&42)), encode(Some(42)));
    }

    #[test]
    fn test_pair() {
        assert_eq!(encode((5, 2)), Ok("( 05 02".to_owned()));
        assert_eq!(
            encode((-72, (90121, ()))),
            Ok("( b8 ( 016009 NIL".to_owned())
        );
        assert_eq!(
            encode((((), ((), ((), (((), ((), ((), ()))), ())))), ())),
            Ok("( ( NIL ( NIL ( NIL ( ( NIL ( NIL ( NIL NIL NIL NIL".to_owned())
        );
    }

    #[test]
    fn test_nil() {
        assert_eq!(encode(()), Ok("NIL".to_owned()));
    }

    #[test]
    fn test_slice() {
        assert_eq!(
            encode([1, 2, 3, 4].as_slice()),
            Ok("( 01 ( 02 ( 03 ( 04 NIL".to_owned())
        );
        assert_eq!(encode([0; 0].as_slice()), Ok("NIL".to_owned()));
    }

    #[test]
    fn test_array() {
        assert_eq!(
            encode([1, 2, 3, 4]),
            Ok("( 01 ( 02 ( 03 ( 04 NIL".to_owned())
        );
        assert_eq!(encode([0; 0]), Ok("NIL".to_owned()));
    }

    #[test]
    fn test_vec() {
        assert_eq!(
            encode(vec![1, 2, 3, 4]),
            Ok("( 01 ( 02 ( 03 ( 04 NIL".to_owned())
        );
        assert_eq!(encode(vec![0; 0]), Ok("NIL".to_owned()));
    }

    #[test]
    fn test_option() {
        assert_eq!(encode(Some("hello")), Ok("68656c6c6f".to_owned()));
        assert_eq!(encode(None::<&str>), Ok("NIL".to_owned()));
        assert_eq!(encode(Some("")), Ok("NIL".to_owned()));
    }

    #[test]
    fn test_str() {
        assert_eq!(encode("hello"), Ok("68656c6c6f".to_owned()));
        assert_eq!(encode(""), Ok("NIL".to_owned()));
    }

    #[test]
    fn test_string() {
        assert_eq!(encode("hello".to_string()), Ok("68656c6c6f".to_owned()));
        assert_eq!(encode("".to_string()), Ok("NIL".to_owned()));
    }

    #[cfg(feature = "chia-bls")]
    #[test]
    fn test_public_key() {
        use chia_bls::PublicKey;
        use hex_literal::hex;

        let valid_bytes = hex!("b8f7dd239557ff8c49d338f89ac1a258a863fa52cd0a502e3aaae4b6738ba39ac8d982215aa3fa16bc5f8cb7e44b954d");
        assert_eq!(
            encode(PublicKey::from_bytes(&valid_bytes).unwrap()),
            Ok(hex::encode(valid_bytes))
        );
    }

    #[cfg(feature = "chia-bls")]
    #[test]
    fn test_signature() {
        use chia_bls::Signature;
        use hex_literal::hex;

        let valid_bytes = hex!("a3994dc9c0ef41a903d3335f0afe42ba16c88e7881706798492da4a1653cd10c69c841eeb56f44ae005e2bad27fb7ebb16ce8bbfbd708ea91dd4ff24f030497b50e694a8270eccd07dbc206b8ffe0c34a9ea81291785299fae8206a1e1bbc1d1");
        assert_eq!(
            encode(Signature::from_bytes(&valid_bytes).unwrap()),
            Ok(hex::encode(valid_bytes))
        );
    }
}
