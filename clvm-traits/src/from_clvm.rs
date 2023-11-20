use clvmr::{allocator::NodePtr, Allocator};
use num_bigint::{BigInt, Sign};

use crate::{ClvmDecoder, FromClvmError};

pub trait FromClvm<N>: Sized {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError>;
}

pub trait FromNodePtr {
    fn from_node_ptr(a: &Allocator, node: NodePtr) -> Result<Self, FromClvmError>
    where
        Self: Sized;
}

impl<T> FromNodePtr for T
where
    T: FromClvm<NodePtr>,
{
    fn from_node_ptr(a: &Allocator, node: NodePtr) -> Result<Self, FromClvmError>
    where
        Self: Sized,
    {
        T::from_clvm(a, node)
    }
}

impl FromClvm<NodePtr> for NodePtr {
    fn from_clvm(
        _decoder: &impl ClvmDecoder<Node = NodePtr>,
        node: NodePtr,
    ) -> Result<Self, FromClvmError> {
        Ok(node)
    }
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl<N> FromClvm<N> for $primitive {
            fn from_clvm(
                decoder: &impl ClvmDecoder<Node = N>,
                node: N,
            ) -> Result<Self, FromClvmError> {
                const LEN: usize = std::mem::size_of::<$primitive>();

                let bytes = decoder.decode_atom(&node)?;
                let number = BigInt::from_signed_bytes_be(bytes);
                let (sign, mut vec) = number.to_bytes_be();

                if vec.len() < std::mem::size_of::<$primitive>() {
                    let mut zeros = vec![0; LEN - vec.len()];
                    zeros.extend(vec);
                    vec = zeros;
                }

                let value = <$primitive>::from_be_bytes(vec.as_slice().try_into().or(Err(
                    FromClvmError::WrongAtomLength {
                        expected: LEN,
                        found: bytes.len(),
                    },
                ))?);

                Ok(if sign == Sign::Minus {
                    value.wrapping_neg()
                } else {
                    value
                })
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

impl<N, A, B> FromClvm<N> for (A, B)
where
    A: FromClvm<N>,
    B: FromClvm<N>,
{
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let (first, rest) = decoder.decode_pair(&node)?;
        let first = A::from_clvm(decoder, first)?;
        let rest = B::from_clvm(decoder, rest)?;
        Ok((first, rest))
    }
}

impl<N> FromClvm<N> for () {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let bytes = decoder.decode_atom(&node)?;
        if bytes.is_empty() {
            Ok(())
        } else {
            Err(FromClvmError::WrongAtomLength {
                expected: 0,
                found: bytes.len(),
            })
        }
    }
}

impl<N, T, const LEN: usize> FromClvm<N> for [T; LEN]
where
    T: FromClvm<N>,
{
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, mut node: N) -> Result<Self, FromClvmError> {
        let mut items = Vec::with_capacity(LEN);
        loop {
            if let Ok((first, rest)) = decoder.decode_pair(&node) {
                if items.len() >= LEN {
                    return Err(FromClvmError::ExpectedAtom);
                } else {
                    items.push(T::from_clvm(decoder, first)?);
                    node = rest;
                }
            } else {
                let bytes = decoder.decode_atom(&node)?;
                if bytes.is_empty() {
                    return items.try_into().or(Err(FromClvmError::ExpectedPair));
                } else {
                    return Err(FromClvmError::WrongAtomLength {
                        expected: 0,
                        found: bytes.len(),
                    });
                }
            }
        }
    }
}

impl<N, T> FromClvm<N> for Vec<T>
where
    T: FromClvm<N>,
{
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, mut node: N) -> Result<Self, FromClvmError> {
        let mut items = Vec::new();
        loop {
            if let Ok((first, rest)) = decoder.decode_pair(&node) {
                items.push(T::from_clvm(decoder, first)?);
                node = rest;
            } else {
                let bytes = decoder.decode_atom(&node)?;
                if bytes.is_empty() {
                    return Ok(items);
                } else {
                    return Err(FromClvmError::WrongAtomLength {
                        expected: 0,
                        found: bytes.len(),
                    });
                }
            }
        }
    }
}

impl<N, T> FromClvm<N> for Option<T>
where
    T: FromClvm<N>,
{
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        if let Ok(&[]) = decoder.decode_atom(&node) {
            Ok(None)
        } else {
            Ok(Some(T::from_clvm(decoder, node)?))
        }
    }
}

impl<N> FromClvm<N> for String {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let bytes = decoder.decode_atom(&node)?;
        Ok(Self::from_utf8(bytes.to_vec())?)
    }
}

#[cfg(test)]
mod tests {
    use clvmr::{allocator::NodePtr, serde::node_from_bytes, Allocator};

    use super::*;

    fn decode<T>(a: &mut Allocator, hex: &str) -> Result<T, FromClvmError>
    where
        T: FromClvm<NodePtr>,
    {
        let bytes = hex::decode(hex).unwrap();
        let actual = node_from_bytes(a, &bytes).unwrap();
        T::from_clvm(a, actual)
    }

    #[test]
    fn test_nodeptr() {
        let a = &mut Allocator::new();
        let ptr = a.one();
        assert_eq!(NodePtr::from_clvm(a, ptr).unwrap(), ptr);
    }

    #[test]
    fn test_primitives() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "80"), Ok(0u8));
        assert_eq!(decode(a, "80"), Ok(0i8));
        assert_eq!(decode(a, "05"), Ok(5u8));
        assert_eq!(decode(a, "05"), Ok(5u32));
        assert_eq!(decode(a, "05"), Ok(5i32));
        assert_eq!(decode(a, "81e5"), Ok(-27i32));
        assert_eq!(decode(a, "80"), Ok(-0));
        assert_eq!(decode(a, "8180"), Ok(-128i8));
    }

    #[test]
    fn test_pair() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "ff0502"), Ok((5, 2)));
        assert_eq!(decode(a, "ff81b8ff8301600980"), Ok((-72, (90121, ()))));
        assert_eq!(
            decode(a, "ffff80ff80ff80ffff80ff80ff80808080"),
            Ok((((), ((), ((), (((), ((), ((), ()))), ())))), ()))
        );
    }

    #[test]
    fn test_nil() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "80"), Ok(()));
    }

    #[test]
    fn test_array() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "ff01ff02ff03ff0480"), Ok([1, 2, 3, 4]));
        assert_eq!(decode(a, "80"), Ok([] as [i32; 0]));
    }

    #[test]
    fn test_vec() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "ff01ff02ff03ff0480"), Ok(vec![1, 2, 3, 4]));
        assert_eq!(decode(a, "80"), Ok(Vec::<i32>::new()));
    }

    #[test]
    fn test_option() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "8568656c6c6f"), Ok(Some("hello".to_string())));
        assert_eq!(decode(a, "80"), Ok(None::<String>));

        // Empty strings get decoded as None instead, since both values are represented by nil bytes.
        // This could be considered either intended behavior or not, depending on the way it's used.
        assert_ne!(decode(a, "80"), Ok(Some("".to_string())));
    }

    #[test]
    fn test_string() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "8568656c6c6f"), Ok("hello".to_string()));
        assert_eq!(decode(a, "80"), Ok("".to_string()));
    }
}
