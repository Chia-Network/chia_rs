use std::{rc::Rc, sync::Arc};

use clvmr::Atom;
use num_bigint::BigInt;

use crate::{encode_number, ClvmEncoder, ToClvmError};

pub trait ToClvm<E>
where
    E: ClvmEncoder,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<E::Node, ToClvmError>;
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for $primitive {
            fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
                let bytes = self.to_be_bytes();
                #[allow(unused_comparisons)]
                encoder.encode_atom(Atom::Borrowed(&encode_number(&bytes, *self < 0)))
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

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for BigInt {
    fn to_clvm(&self, encoder: &mut E) -> Result<<E as ClvmEncoder>::Node, ToClvmError> {
        encoder.encode_bigint(self.clone())
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for bool {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        i32::from(*self).to_clvm(encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for &T
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        T::to_clvm(*self, encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for Box<T>
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        T::to_clvm(self, encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for Rc<T>
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        T::to_clvm(self, encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for Arc<T>
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        T::to_clvm(self, encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, A, B> ToClvm<E> for (A, B)
where
    A: ToClvm<E>,
    B: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        let first = self.0.to_clvm(encoder)?;
        let rest = self.1.to_clvm(encoder)?;
        encoder.encode_pair(first, rest)
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for () {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        encoder.encode_atom(Atom::Borrowed(&[]))
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for &[T]
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        let mut result = encoder.encode_atom(Atom::Borrowed(&[]))?;
        for item in self.iter().rev() {
            let value = item.to_clvm(encoder)?;
            result = encoder.encode_pair(value, result)?;
        }
        Ok(result)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T, const LEN: usize> ToClvm<E> for [T; LEN]
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        self.as_slice().to_clvm(encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for Vec<T>
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        self.as_slice().to_clvm(encoder)
    }
}

impl<N, E: ClvmEncoder<Node = N>, T> ToClvm<E> for Option<T>
where
    T: ToClvm<E>,
{
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        match self {
            Some(value) => value.to_clvm(encoder),
            None => encoder.encode_atom(Atom::Borrowed(&[])),
        }
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for &str {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        encoder.encode_atom(Atom::Borrowed(self.as_bytes()))
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for String {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        self.as_str().to_clvm(encoder)
    }
}

#[cfg(feature = "chia-bls")]
impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for chia_bls::PublicKey {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        encoder.encode_atom(Atom::Borrowed(&self.to_bytes()))
    }
}

#[cfg(feature = "chia-bls")]
impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for chia_bls::Signature {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        encoder.encode_atom(Atom::Borrowed(&self.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use clvmr::{serde::node_to_bytes, Allocator};
    use hex::ToHex;

    use super::*;

    fn encode<T>(a: &mut Allocator, value: T) -> Result<String, ToClvmError>
    where
        T: ToClvm<Allocator>,
    {
        let actual = value.to_clvm(a)?;
        let actual_bytes = node_to_bytes(a, actual).unwrap();
        Ok(actual_bytes.encode_hex())
    }

    #[test]
    fn test_nodeptr() {
        let a = &mut Allocator::new();
        let ptr = a.one();
        assert_eq!(ptr.to_clvm(a).unwrap(), ptr);
    }

    #[test]
    fn test_primitives() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, 0u8), Ok("80".to_owned()));
        assert_eq!(encode(a, 0i8), Ok("80".to_owned()));
        assert_eq!(encode(a, 5u8), Ok("05".to_owned()));
        assert_eq!(encode(a, 5u32), Ok("05".to_owned()));
        assert_eq!(encode(a, 5i32), Ok("05".to_owned()));
        assert_eq!(encode(a, -27i32), Ok("81e5".to_owned()));
        assert_eq!(encode(a, -0), Ok("80".to_owned()));
        assert_eq!(encode(a, -128i8), Ok("8180".to_owned()));
    }

    #[test]
    fn test_bool() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, true), Ok("01".to_owned()));
        assert_eq!(encode(a, false), Ok("80".to_owned()));
    }

    #[test]
    fn test_reference() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, [1, 2, 3]), encode(a, [1, 2, 3]));
        assert_eq!(encode(a, Some(42)), encode(a, Some(42)));
        assert_eq!(encode(a, Some(&42)), encode(a, Some(42)));
        assert_eq!(encode(a, Some(&42)), encode(a, Some(42)));
    }

    #[test]
    fn test_smart_pointers() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, Box::new(42)), encode(a, 42));
        assert_eq!(encode(a, Rc::new(42)), encode(a, 42));
        assert_eq!(encode(a, Arc::new(42)), encode(a, 42));
    }

    #[test]
    fn test_pair() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, (5, 2)), Ok("ff0502".to_owned()));
        assert_eq!(
            encode(a, (-72, (90121, ()))),
            Ok("ff81b8ff8301600980".to_owned())
        );
        assert_eq!(
            encode(a, (((), ((), ((), (((), ((), ((), ()))), ())))), ())),
            Ok("ffff80ff80ff80ffff80ff80ff80808080".to_owned())
        );
    }

    #[test]
    fn test_nil() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, ()), Ok("80".to_owned()));
    }

    #[test]
    fn test_slice() {
        let a = &mut Allocator::new();
        assert_eq!(
            encode(a, [1, 2, 3, 4].as_slice()),
            Ok("ff01ff02ff03ff0480".to_owned())
        );
        assert_eq!(encode(a, [0; 0].as_slice()), Ok("80".to_owned()));
    }

    #[test]
    fn test_array() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, [1, 2, 3, 4]), Ok("ff01ff02ff03ff0480".to_owned()));
        assert_eq!(encode(a, [0; 0]), Ok("80".to_owned()));
    }

    #[test]
    fn test_vec() {
        let a = &mut Allocator::new();
        assert_eq!(
            encode(a, vec![1, 2, 3, 4]),
            Ok("ff01ff02ff03ff0480".to_owned())
        );
        assert_eq!(encode(a, vec![0; 0]), Ok("80".to_owned()));
    }

    #[test]
    fn test_option() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, Some("hello")), Ok("8568656c6c6f".to_owned()));
        assert_eq!(encode(a, None::<&str>), Ok("80".to_owned()));
        assert_eq!(encode(a, Some("")), Ok("80".to_owned()));
    }

    #[test]
    fn test_str() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, "hello"), Ok("8568656c6c6f".to_owned()));
        assert_eq!(encode(a, ""), Ok("80".to_owned()));
    }

    #[test]
    fn test_string() {
        let a = &mut Allocator::new();
        assert_eq!(
            encode(a, "hello".to_string()),
            Ok("8568656c6c6f".to_owned())
        );
        assert_eq!(encode(a, String::new()), Ok("80".to_owned()));
    }

    #[cfg(feature = "chia-bls")]
    #[test]
    fn test_public_key() {
        use chia_bls::PublicKey;
        use hex_literal::hex;

        let a = &mut Allocator::new();

        let bytes = hex!(
            "
            b8f7dd239557ff8c49d338f89ac1a258a863fa52cd0a502e
            3aaae4b6738ba39ac8d982215aa3fa16bc5f8cb7e44b954d
            "
        );
        assert_eq!(
            encode(a, PublicKey::from_bytes(&bytes).unwrap()),
            Ok("b0b8f7dd239557ff8c49d338f89ac1a258a863fa52cd0a502e3aaae4b6738ba39ac8d982215aa3fa16bc5f8cb7e44b954d".to_string())
        );
    }

    #[cfg(feature = "chia-bls")]
    #[test]
    fn test_signature() {
        use chia_bls::Signature;
        use hex_literal::hex;

        let a = &mut Allocator::new();

        let bytes = hex!(
            "
            a3994dc9c0ef41a903d3335f0afe42ba16c88e7881706798492da4a1653cd10c
            69c841eeb56f44ae005e2bad27fb7ebb16ce8bbfbd708ea91dd4ff24f030497b
            50e694a8270eccd07dbc206b8ffe0c34a9ea81291785299fae8206a1e1bbc1d1
            "
        );
        assert_eq!(
            encode(a, Signature::from_bytes(&bytes).unwrap()),
            Ok("c060a3994dc9c0ef41a903d3335f0afe42ba16c88e7881706798492da4a1653cd10c69c841eeb56f44ae005e2bad27fb7ebb16ce8bbfbd708ea91dd4ff24f030497b50e694a8270eccd07dbc206b8ffe0c34a9ea81291785299fae8206a1e1bbc1d1".to_string())
        );
    }
}
