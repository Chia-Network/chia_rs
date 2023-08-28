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
                    let value = <$primitive>::from_be_bytes(vec.as_slice().try_into()?);
                    Ok(if sign == Sign::Minus {
                        value.wrapping_neg()
                    } else {
                        value
                    })
                } else {
                    Err(Error::msg("expected atom"))
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
            SExp::Atom => Err(Error::msg("expected atom")),
        }
    }
}

impl FromClvm for () {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        if nullp(a, ptr) {
            Ok(())
        } else {
            Err(Error::msg("expected nil"))
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
                            Err(_) => Err(Error::msg("expected cons")),
                        };
                    } else {
                        return Err(Error::msg("expected nil"));
                    }
                }
                SExp::Pair(first, rest) => {
                    if items.len() >= N {
                        return Err(Error::msg("expected atom"));
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
                        return Err(Error::msg("expected nil"));
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
            Ok(Self::from_utf8(a.atom(ptr).to_vec())?)
        } else {
            Err(Error::msg("expected atom"))
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

    fn check<T>(a: &mut Allocator, hex: &str, value: T)
    where
        T: FromClvm + PartialEq + std::fmt::Debug,
    {
        let result = decode::<T>(a, hex).unwrap();
        assert_eq!(result, value);
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
        check(a, "80", 0u8);
        check(a, "80", 0i8);
        check(a, "05", 5u8);
        check(a, "05", 5u32);
        check(a, "05", 5i32);
        check(a, "81e5", -27i32);
        check(a, "80", -0i32);
        check(a, "8180", -128i8);
    }

    #[test]
    fn test_pair() {
        let a = &mut Allocator::new();
        check(a, "ff0502", (5, 2));
        check(a, "ff81b8ff8301600980", (-72, (90121, ())));
        check(
            a,
            "ffff80ff80ff80ffff80ff80ff80808080",
            (((), ((), ((), (((), ((), ((), ()))), ())))), ()),
        );
    }

    #[test]
    fn test_nil() {
        let a = &mut Allocator::new();
        check(a, "80", ());
    }

    #[test]
    fn test_array() {
        let a = &mut Allocator::new();
        check(a, "ff01ff02ff03ff0480", [1, 2, 3, 4]);
        check::<[i32; 0]>(a, "80", []);
    }

    #[test]
    fn test_vec() {
        let a = &mut Allocator::new();
        check(a, "ff01ff02ff03ff0480", vec![1, 2, 3, 4]);
        check(a, "80", Vec::<i32>::new());
    }

    #[test]
    fn test_option() {
        let a = &mut Allocator::new();
        check(a, "8568656c6c6f", Some("hello".to_string()));

        // Empty strings get decoded as None instead, since both values are represented by nil bytes.
        // This could be considered either intended behavior or not, depending on the way it's used.
        check(a, "80", None::<String>);
    }

    #[test]
    fn test_string() {
        let a = &mut Allocator::new();
        check(a, "8568656c6c6f", "hello".to_string());
        check(a, "80", "".to_string());
    }
}
