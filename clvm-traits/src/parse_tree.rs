use anyhow::{Error, Result};
use clvmr::allocator::NodePtr;
use num_bigint::{BigInt, Sign};

use crate::Value;

pub trait ParseTree<N>: Sized {
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self>;
}

impl ParseTree<NodePtr> for NodePtr {
    fn parse_tree<'a>(_f: &impl Fn(NodePtr) -> Value<'a, NodePtr>, ptr: NodePtr) -> Result<Self> {
        Ok(ptr)
    }
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl<N> ParseTree<N> for $primitive {
            fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self> {
                if let Value::Atom(atom) = f(ptr) {
                    let value = BigInt::from_signed_bytes_be(atom);
                    let (sign, mut vec) = value.to_bytes_be();
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

impl<N, A, B> ParseTree<N> for (A, B)
where
    A: ParseTree<N>,
    B: ParseTree<N>,
{
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self> {
        if let Value::Pair(first, rest) = f(ptr) {
            Ok((A::parse_tree(f, first)?, B::parse_tree(f, rest)?))
        } else {
            Err(Error::msg("expected atom"))
        }
    }
}

impl<N> ParseTree<N> for () {
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self> {
        if let Value::Atom(&[]) = f(ptr) {
            Ok(())
        } else {
            Err(Error::msg("expected nil"))
        }
    }
}

impl<N, T, const LEN: usize> ParseTree<N> for [T; LEN]
where
    T: ParseTree<N>,
{
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, mut ptr: N) -> Result<Self> {
        let mut items = Vec::with_capacity(LEN);
        loop {
            match f(ptr) {
                Value::Atom(&[]) => {
                    return items.try_into().map_err(|_| Error::msg("expected cons"))
                }
                Value::Atom(_) => return Err(Error::msg("expected nil")),
                Value::Pair(first, rest) => {
                    if items.len() >= LEN {
                        return Err(Error::msg("expected atom"));
                    } else {
                        items.push(T::parse_tree(f, first)?);
                        ptr = rest;
                    }
                }
            }
        }
    }
}

impl<N, T> ParseTree<N> for Vec<T>
where
    T: ParseTree<N>,
{
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, mut ptr: N) -> Result<Self> {
        let mut items = Vec::new();
        loop {
            match f(ptr) {
                Value::Atom(&[]) => return Ok(items),
                Value::Atom(_) => return Err(Error::msg("expected nil")),
                Value::Pair(first, rest) => {
                    items.push(T::parse_tree(f, first)?);
                    ptr = rest;
                }
            }
        }
    }
}

impl<N, T> ParseTree<N> for Option<T>
where
    N: Copy,
    T: ParseTree<N>,
{
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self> {
        if let Value::Atom(&[]) = f(ptr) {
            Ok(None)
        } else {
            Ok(Some(T::parse_tree(f, ptr)?))
        }
    }
}

impl<N> ParseTree<N> for String {
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self> {
        if let Value::Atom(atom) = f(ptr) {
            Ok(Self::from_utf8(atom.to_vec())?)
        } else {
            Err(Error::msg("expected atom"))
        }
    }
}
