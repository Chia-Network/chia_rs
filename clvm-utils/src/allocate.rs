use std::{array::TryFromSliceError, io::Cursor};

use chia_bls::PublicKey;
use chia_protocol::{chia_error, Program, Streamable};
use clvmr::{
    allocator::{NodePtr, SExp},
    op_utils::nullp,
    reduction::EvalErr,
    serde::{node_from_bytes, node_to_bytes},
    Allocator,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Reason(String),

    #[error("{0:?}")]
    Eval(EvalErr),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Protocol(#[from] chia_error::Error),

    #[error("expected atom")]
    ExpectedAtom(NodePtr),

    #[error("expected cons")]
    ExpectedCons(NodePtr),

    #[error("expected nil")]
    ExpectedNil(NodePtr),

    #[error("validation failed")]
    Validation(NodePtr),
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Allocate: Sized {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self>;
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr>;
}

macro_rules! allocate_primitive {
    ($t:ty) => {
        impl Allocate for $t {
            fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
                if let SExp::Atom() = a.sexp(node) {
                    let mut vec = a.atom(node).to_vec();
                    if vec.len() < std::mem::size_of::<$t>() {
                        let mut zeros = vec![0; std::mem::size_of::<$t>() - vec.len()];
                        zeros.extend(vec);
                        vec = zeros;
                    }
                    Ok(<$t>::from_be_bytes(vec.as_slice().try_into().map_err(
                        |error: TryFromSliceError| Error::Reason(error.to_string()),
                    )?))
                } else {
                    Err(Error::ExpectedAtom(node))
                }
            }
            fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
                a.new_number((*self).into()).map_err(Error::Eval)
            }
        }
    };
}

allocate_primitive!(u8);
allocate_primitive!(i8);
allocate_primitive!(u16);
allocate_primitive!(i16);
allocate_primitive!(u32);
allocate_primitive!(i32);
allocate_primitive!(u64);
allocate_primitive!(i64);
allocate_primitive!(u128);
allocate_primitive!(i128);

impl<A, B> Allocate for (A, B)
where
    A: Allocate,
    B: Allocate,
{
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        match a.sexp(node) {
            SExp::Pair(first, rest) => Ok((A::from_clvm(a, first)?, B::from_clvm(a, rest)?)),
            SExp::Atom() => Err(Error::ExpectedCons(node)),
        }
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let first = self.0.to_clvm(a)?;
        let rest = self.1.to_clvm(a)?;
        a.new_pair(first, rest).map_err(Error::Eval)
    }
}

impl<T> Allocate for Vec<T>
where
    T: Allocate,
{
    fn from_clvm(a: &Allocator, mut node: NodePtr) -> Result<Self> {
        let mut list = Vec::new();
        while let SExp::Pair(first, rest) = a.sexp(node) {
            list.push(T::from_clvm(a, first)?);
            node = rest;
        }
        if nullp(a, node) {
            Ok(list)
        } else {
            Err(Error::ExpectedNil(node))
        }
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let mut list = a.null();
        for item in self.iter().rev() {
            let item_ptr = item.to_clvm(a)?;
            list = a.new_pair(item_ptr, list).map_err(Error::Eval)?;
        }
        Ok(list)
    }
}

impl<const N: usize> Allocate for [u8; N] {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom() = a.sexp(node) {
            Ok(a.atom(node)
                .try_into()
                .map_err(|error: TryFromSliceError| Error::Reason(error.to_string()))?)
        } else {
            Err(Error::ExpectedAtom(node))
        }
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        a.new_atom(self).map_err(Error::Eval)
    }
}

impl Allocate for Program {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        Ok(Program::parse(&mut Cursor::new(&node_to_bytes(a, node)?))?)
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let mut bytes = Vec::new();
        self.stream(&mut bytes)?;
        Ok(node_from_bytes(a, &bytes)?)
    }
}

impl Allocate for PublicKey {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        let bytes = <[u8; 48]>::from_clvm(a, node)?;
        PublicKey::validate(&bytes).ok_or_else(|| Error::Validation(node))
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        self.to_bytes().to_clvm(a)
    }
}
