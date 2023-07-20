use std::array::TryFromSliceError;

use clvmr::{
    allocator::{NodePtr, SExp},
    op_utils::nullp,
    Allocator,
};
use num_bigint::{BigInt, Sign};

use crate::{Error, Result};

pub trait ToClvm: Sized {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr>;
}

pub trait FromClvm: Sized {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self>;
}

#[macro_export]
macro_rules! clvm_list {
    () => {
        ()
    };
    ( $first:expr $( , $rest:expr )* $(,)? ) => {
        ($first, $crate::clvm_list!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! clvm_tuple {
    ( $first:expr $(,)? ) => {
        $first
    };
    ( $first:expr $( , $rest:expr )* $(,)? ) => {
        ($first, $crate::clvm_tuple!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! clvm_quote {
    ( $value:expr ) => {
        (1u8, $value)
    };
}

#[macro_export]
macro_rules! clvm_curried_args {
    () => {
        1u8
    };
    ( $first:expr $( , $rest:expr )* $(,)? ) => {
        (4u8, ($crate::clvm_quote!($first), ($crate::clvm_curried_args!( $( $rest ),* ), ())))
    };
}

#[macro_export]
macro_rules! match_list {
    () => {
        $crate::MatchByte::<0>
    };
    ( $first:ty $( , $rest:ty )* $(,)? ) => {
        ($first, $crate::match_list!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! match_tuple {
    ( $first:ty $(,)? ) => {
        $first
    };
    ( $first:ty $( , $rest:ty )* $(,)? ) => {
        ($first, $crate::match_tuple!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! match_quote {
    ( $type:ty ) => {
        ($crate::MatchByte::<1>, $type)
    };
}

#[macro_export]
macro_rules! match_curried_args {
    () => {
        $crate::MatchByte::<1>
    };
    ( $first:ty $( , $rest:ty )* $(,)? ) => {
        (
            $crate::MatchByte::<4>,
            (
                $crate::match_quote!($first),
                ($crate::match_curried_args!( $( $rest ),* ), ()),
            ),
        )
    };
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl ToClvm for $primitive {
            fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
                a.new_number((*self).into()).map_err(Error::Allocator)
            }
        }

        impl FromClvm for $primitive {
            fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
                if let SExp::Atom() = a.sexp(node) {
                    let (sign, mut vec) = a.number(node).to_bytes_be();
                    if vec.len() < std::mem::size_of::<$primitive>() {
                        let mut zeros = vec![0; std::mem::size_of::<$primitive>() - vec.len()];
                        zeros.extend(vec);
                        vec = zeros;
                    }
                    let value =
                        <$primitive>::from_be_bytes(vec.as_slice().try_into().map_err(
                            |error: TryFromSliceError| Error::Reason(error.to_string()),
                        )?);
                    Ok(if sign == Sign::Minus {
                        value.wrapping_neg()
                    } else {
                        value
                    })
                } else {
                    Err(Error::ExpectedAtom(node))
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

impl<A, B> ToClvm for (A, B)
where
    A: ToClvm,
    B: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let first = self.0.to_clvm(a)?;
        let rest = self.1.to_clvm(a)?;
        a.new_pair(first, rest).map_err(Error::Allocator)
    }
}

impl<A, B> FromClvm for (A, B)
where
    A: FromClvm,
    B: FromClvm,
{
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        match a.sexp(node) {
            SExp::Pair(first, rest) => Ok((A::from_clvm(a, first)?, B::from_clvm(a, rest)?)),
            SExp::Atom() => Err(Error::ExpectedCons(node)),
        }
    }
}

impl ToClvm for () {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        Ok(a.null())
    }
}

impl FromClvm for () {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if !nullp(a, node) {
            Err(Error::ExpectedNil(node))
        } else {
            Ok(())
        }
    }
}

pub struct MatchByte<const BYTE: u8>;

impl<const BYTE: u8> ToClvm for MatchByte<BYTE> {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        a.new_number(BYTE.into()).map_err(Error::Allocator)
    }
}

impl<const BYTE: u8> FromClvm for MatchByte<BYTE> {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom() = a.sexp(node) {
            let value: u8 =
                a.number(node)
                    .try_into()
                    .map_err(|error: <u8 as TryFrom<BigInt>>::Error| {
                        Error::Reason(error.to_string())
                    })?;

            if value == BYTE {
                Ok(Self)
            } else {
                Err(Error::Reason(format!("expected {}", BYTE)))
            }
        } else {
            Err(Error::ExpectedAtom(node))
        }
    }
}
