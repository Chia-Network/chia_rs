use clvmr::allocator::NodePtr;
use num_bigint::BigInt;

use crate::{ClvmValue, ToClvmError};

/// A trait for converting a Rust value to a CLVM value.
pub trait ToClvm<Node>
where
    Node: Clone,
{
    fn to_clvm(
        &self,
        f: &mut impl FnMut(ClvmValue<Node>) -> Result<Node, ToClvmError>,
    ) -> Result<Node, ToClvmError>;
}

#[macro_export]
macro_rules! to_clvm {
    ( $node:ty, $self:ident, $f:ident, { $( $block:tt )* } ) => {
        #[allow(unused_mut)]
        fn to_clvm(
            &$self,
            mut $f: &mut impl FnMut($crate::ClvmValue<$node>) -> ::std::result::Result<$node, $crate::ToClvmError>,
        ) -> ::std::result::Result<$node, $crate::ToClvmError> {
            $( $block )*
        }
    };
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

macro_rules! clvm_ints {
    ($int:ty) => {
        impl<Node> ToClvm<Node> for $int
        where
            Node: Clone,
        {
            to_clvm!(Node, self, f, {
                let bytes = BigInt::from(*self);
                f(ClvmValue::Atom(simplify_int_bytes(
                    &bytes.to_signed_bytes_be(),
                )))
            });
        }
    };
}

clvm_ints!(u8);
clvm_ints!(i8);
clvm_ints!(u16);
clvm_ints!(i16);
clvm_ints!(u32);
clvm_ints!(i32);
clvm_ints!(u64);
clvm_ints!(i64);
clvm_ints!(u128);
clvm_ints!(i128);
clvm_ints!(usize);
clvm_ints!(isize);

impl ToClvm<NodePtr> for NodePtr {
    to_clvm!(NodePtr, self, _f, { Ok(*self) });
}

impl<Node, T> ToClvm<Node> for &T
where
    Node: Clone,
    T: ToClvm<Node>,
{
    to_clvm!(Node, self, f, { (*self).to_clvm(f) });
}

impl<Node, A, B> ToClvm<Node> for (A, B)
where
    Node: Clone,
    A: ToClvm<Node>,
    B: ToClvm<Node>,
{
    to_clvm!(Node, self, f, {
        let first = self.0.to_clvm(f)?;
        let rest = self.1.to_clvm(f)?;
        f(ClvmValue::Pair(first, rest))
    });
}

impl<Node> ToClvm<Node> for ()
where
    Node: Clone,
{
    to_clvm!(Node, self, f, { f(ClvmValue::Atom(&[])) });
}

impl<Node, T> ToClvm<Node> for &[T]
where
    Node: Clone,
    T: ToClvm<Node>,
{
    to_clvm!(Node, self, f, {
        let mut result = f(ClvmValue::Atom(&[]))?;
        for item in self.iter().rev() {
            let value = item.to_clvm(f)?;
            result = f(ClvmValue::Pair(value, result))?;
        }
        Ok(result)
    });
}

impl<Node, T, const N: usize> ToClvm<Node> for [T; N]
where
    Node: Clone,
    T: ToClvm<Node>,
{
    to_clvm!(Node, self, f, { self.as_slice().to_clvm(f) });
}

impl<Node, T> ToClvm<Node> for Vec<T>
where
    Node: Clone,
    T: ToClvm<Node>,
{
    to_clvm!(Node, self, f, { self.as_slice().to_clvm(f) });
}

impl<Node, T> ToClvm<Node> for Option<T>
where
    Node: Clone,
    T: ToClvm<Node>,
{
    to_clvm!(Node, self, f, {
        match self {
            Some(value) => value.to_clvm(f),
            None => f(ClvmValue::Atom(&[])),
        }
    });
}

impl<Node> ToClvm<Node> for &str
where
    Node: Clone,
{
    to_clvm!(Node, self, f, { f(ClvmValue::Atom(self.as_bytes())) });
}

impl<Node> ToClvm<Node> for String
where
    Node: Clone,
{
    to_clvm!(Node, self, f, { self.as_str().to_clvm(f) });
}

#[cfg(test)]
mod tests {
    use clvmr::{serde::node_to_bytes, Allocator};
    use hex::ToHex;

    use crate::ToPtr;

    use super::*;

    fn encode<T>(a: &mut Allocator, value: T) -> Result<String, ToClvmError>
    where
        T: ToPtr,
    {
        let actual = value.to_ptr(a).unwrap();
        let actual_bytes = node_to_bytes(a, actual).unwrap();
        Ok(actual_bytes.encode_hex())
    }

    #[test]
    fn test_nodeptr() {
        let a = &mut Allocator::new();
        let ptr = a.one();
        assert_eq!(ptr.to_ptr(a).unwrap(), ptr);
    }

    #[test]
    fn test_ints() {
        let a = &mut Allocator::new();
        assert_eq!(encode(a, 0u8), Ok("80".to_owned()));
        assert_eq!(encode(a, 0i8), Ok("80".to_owned()));
        assert_eq!(encode(a, 5u8), Ok("05".to_owned()));
        assert_eq!(encode(a, 5u32), Ok("05".to_owned()));
        assert_eq!(encode(a, 5i32), Ok("05".to_owned()));
        assert_eq!(encode(a, -27i32), Ok("81e5".to_owned()));
        assert_eq!(encode(a, -0), Ok("80".to_owned()));
        assert_eq!(encode(a, -128i8), Ok("8180".to_owned()));
        assert_eq!(encode(a, 1000000000000u64), Ok("8600e8d4a51000".to_owned()));
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
        assert_eq!(encode(a, "".to_string()), Ok("80".to_owned()));
    }
}
