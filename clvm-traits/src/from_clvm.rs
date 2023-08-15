use std::array::TryFromSliceError;

use clvmr::{
    allocator::{NodePtr, SExp},
    op_utils::nullp,
    Allocator,
};
use num_bigint::Sign;

use crate::{Error, Result};

pub trait FromClvm: Sized {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self>;
}

impl FromClvm for NodePtr {
    fn from_clvm(_a: &Allocator, ptr: NodePtr) -> Result<Self> {
        Ok(ptr)
    }
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl FromClvm for $primitive {
            fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
                if let SExp::Atom = a.sexp(ptr) {
                    let (sign, mut vec) = a.number(ptr).to_bytes_be();
                    if vec.len() < std::mem::size_of::<$primitive>() {
                        let mut zeros = vec![0; std::mem::size_of::<$primitive>() - vec.len()];
                        zeros.extend(vec);
                        vec = zeros;
                    }
                    let value =
                        <$primitive>::from_be_bytes(vec.as_slice().try_into().map_err(
                            |error: TryFromSliceError| Error::Custom(error.to_string()),
                        )?);
                    Ok(if sign == Sign::Minus {
                        value.wrapping_neg()
                    } else {
                        value
                    })
                } else {
                    Err(Error::ExpectedAtom(ptr))
                }
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

impl<A, B> FromClvm for (A, B)
where
    A: FromClvm,
    B: FromClvm,
{
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        match a.sexp(ptr) {
            SExp::Pair(first, rest) => Ok((A::from_clvm(a, first)?, B::from_clvm(a, rest)?)),
            SExp::Atom => Err(Error::ExpectedCons(ptr)),
        }
    }
}

impl FromClvm for () {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        if nullp(a, ptr) {
            Ok(())
        } else {
            Err(Error::ExpectedNil(ptr))
        }
    }
}

impl<T, const N: usize> FromClvm for [T; N]
where
    T: FromClvm,
{
    fn from_clvm(a: &Allocator, mut ptr: NodePtr) -> Result<Self> {
        let mut items = Vec::with_capacity(N);
        loop {
            match a.sexp(ptr) {
                SExp::Atom => {
                    if nullp(a, ptr) {
                        return match items.try_into() {
                            Ok(value) => Ok(value),
                            Err(_) => Err(Error::ExpectedCons(ptr)),
                        };
                    } else {
                        return Err(Error::ExpectedNil(ptr));
                    }
                }
                SExp::Pair(first, rest) => {
                    if items.len() >= N {
                        return Err(Error::ExpectedAtom(ptr));
                    } else {
                        items.push(T::from_clvm(a, first)?);
                        ptr = rest;
                    }
                }
            }
        }
    }
}

impl<T> FromClvm for Vec<T>
where
    T: FromClvm,
{
    fn from_clvm(a: &Allocator, mut ptr: NodePtr) -> Result<Self> {
        let mut items = Vec::new();
        loop {
            match a.sexp(ptr) {
                SExp::Atom => {
                    if nullp(a, ptr) {
                        return Ok(items);
                    } else {
                        return Err(Error::ExpectedNil(ptr));
                    }
                }
                SExp::Pair(first, rest) => {
                    items.push(T::from_clvm(a, first)?);
                    ptr = rest;
                }
            }
        }
    }
}

impl<T: FromClvm> FromClvm for Option<T> {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        if nullp(a, ptr) {
            Ok(None)
        } else {
            Ok(Some(T::from_clvm(a, ptr)?))
        }
    }
}

impl FromClvm for String {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        if let SExp::Atom = a.sexp(ptr) {
            Self::from_utf8(a.atom(ptr).to_vec()).map_err(|error| Error::Custom(error.to_string()))
        } else {
            Err(Error::ExpectedAtom(ptr))
        }
    }
}

#[cfg(test)]
mod tests {
    use clvmr::serde::node_from_bytes;

    use super::*;

    fn decode<T>(a: &mut Allocator, hex: &str) -> Result<T>
    where
        T: FromClvm,
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
