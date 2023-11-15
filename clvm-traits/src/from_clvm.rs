use clvmr::allocator::NodePtr;

use crate::{ClvmValue, FromClvmError};

/// A trait for converting a CLVM value to a Rust value.
pub trait FromClvm<Node>: Sized
where
    Node: Clone,
{
    fn from_clvm<'a>(
        f: &mut impl FnMut(&Node) -> ClvmValue<'a, Node>,
        ptr: Node,
    ) -> Result<Self, FromClvmError>;
}

#[macro_export]
macro_rules! from_clvm {
    ($node:ty, $f:ident, $ptr:ident, { $( $block:tt )* }) => {
        #[allow(unused_mut)]
        fn from_clvm<'a>(
            mut $f: &mut impl FnMut(&$node) -> $crate::ClvmValue<'a, $node>,
            mut $ptr: $node,
        ) -> ::std::result::Result<Self, $crate::FromClvmError> {
            $( $block )*
        }
    };
}

macro_rules! clvm_ints {
    ($int:ty) => {
        impl<Node> FromClvm<Node> for $int
        where
            Node: Clone,
        {
            from_clvm!(Node, f, ptr, {
                if let ClvmValue::Atom(bytes) = f(&ptr) {
                    const LEN: usize = std::mem::size_of::<$int>();
                    if bytes.len() > LEN {
                        return Err(FromClvmError::ValueTooLarge);
                    }

                    let is_negative = !bytes.is_empty() && (bytes[0] & 0x80) != 0;
                    let fill_byte = if is_negative { 0xFF } else { 0x00 };

                    let mut buf = [fill_byte; LEN];
                    let start = LEN - bytes.len();
                    buf[start..].copy_from_slice(bytes);

                    Ok(<$int>::from_be_bytes(buf))
                } else {
                    Err(FromClvmError::ExpectedAtom)
                }
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

impl FromClvm<NodePtr> for NodePtr {
    from_clvm!(NodePtr, _f, ptr, { Ok(ptr) });
}

impl<Node, A, B> FromClvm<Node> for (A, B)
where
    Node: Clone,
    A: FromClvm<Node>,
    B: FromClvm<Node>,
{
    from_clvm!(Node, f, ptr, {
        if let ClvmValue::Pair(first, rest) = f(&ptr) {
            let first = A::from_clvm(f, first)?;
            let rest = B::from_clvm(f, rest)?;
            Ok((first, rest))
        } else {
            Err(FromClvmError::ExpectedPair)
        }
    });
}

impl<Node> FromClvm<Node> for ()
where
    Node: Clone,
{
    from_clvm!(Node, f, ptr, {
        if let ClvmValue::Atom(&[]) = f(&ptr) {
            Ok(())
        } else {
            Err(FromClvmError::ExpectedNil)
        }
    });
}

impl<Node, T, const N: usize> FromClvm<Node> for [T; N]
where
    Node: Clone,
    T: FromClvm<Node>,
{
    from_clvm!(Node, f, ptr, {
        let mut items = Vec::with_capacity(N);
        loop {
            match f(&ptr) {
                ClvmValue::Atom(&[]) => {
                    return items.try_into().map_err(|_| FromClvmError::ExpectedPair);
                }
                ClvmValue::Atom(_) => {
                    return Err(FromClvmError::ExpectedNil);
                }
                ClvmValue::Pair(first, rest) => {
                    if items.len() == N {
                        return Err(FromClvmError::ExpectedAtom);
                    } else {
                        items.push(T::from_clvm(f, first)?);
                        ptr = rest;
                    }
                }
            }
        }
    });
}

impl<Node, T> FromClvm<Node> for Vec<T>
where
    Node: Clone,
    T: FromClvm<Node>,
{
    from_clvm!(Node, f, ptr, {
        let mut items = Vec::new();
        loop {
            match f(&ptr) {
                ClvmValue::Atom(&[]) => {
                    return Ok(items);
                }
                ClvmValue::Atom(_) => {
                    return Err(FromClvmError::ExpectedNil);
                }
                ClvmValue::Pair(first, rest) => {
                    items.push(T::from_clvm(f, first)?);
                    ptr = rest;
                }
            }
        }
    });
}

impl<Node, T> FromClvm<Node> for Option<T>
where
    Node: Clone,
    T: FromClvm<Node>,
{
    from_clvm!(Node, f, ptr, {
        if let ClvmValue::Atom(&[]) = f(&ptr) {
            Ok(None)
        } else {
            Ok(Some(T::from_clvm(f, ptr)?))
        }
    });
}

impl<Node> FromClvm<Node> for String
where
    Node: Clone,
{
    from_clvm!(Node, f, ptr, {
        if let ClvmValue::Atom(bytes) = f(&ptr) {
            Ok(Self::from_utf8(bytes.to_vec())?)
        } else {
            Err(FromClvmError::ExpectedAtom)
        }
    });
}

#[cfg(test)]
mod tests {
    use clvmr::{serde::node_from_bytes, Allocator};

    use crate::FromPtr;

    use super::*;

    fn decode<T>(a: &mut Allocator, hex: &str) -> Result<T, FromClvmError>
    where
        T: FromPtr,
    {
        let bytes = hex::decode(hex).unwrap();
        let actual = node_from_bytes(a, &bytes).unwrap();
        T::from_ptr(a, actual)
    }

    #[test]
    fn test_nodeptr() {
        let a = &mut Allocator::new();
        let ptr = a.one();
        let value = NodePtr::from_ptr(a, ptr).unwrap();
        assert_eq!(value, ptr);
    }

    #[test]
    fn test_ints() {
        let a = &mut Allocator::new();
        assert_eq!(decode(a, "80"), Ok(0u8));
        assert_eq!(decode(a, "80"), Ok(0i8));
        assert_eq!(decode(a, "05"), Ok(5u8));
        assert_eq!(decode(a, "05"), Ok(5u32));
        assert_eq!(decode(a, "05"), Ok(5i32));
        assert_eq!(decode(a, "81e5"), Ok(-27i32));
        assert_eq!(decode(a, "80"), Ok(-0));
        assert_eq!(decode(a, "8180"), Ok(-128i8));
        assert_eq!(decode(a, "8600e8d4a51000"), Ok(1000000000000u64));
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
