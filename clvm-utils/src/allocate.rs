use std::io::Cursor;

use chia_bls::PublicKey;
use chia_protocol::{Program, Streamable};
use clvmr::{
    allocator::{NodePtr, SExp},
    op_utils::nullp,
    reduction::EvalErr,
    serde::{node_from_bytes, node_to_bytes},
    Allocator,
};

pub trait Allocate: Sized {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self>;
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr>;
}

macro_rules! allocate_primitive {
    ($t:ty) => {
        impl Allocate for $t {
            fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
                if let SExp::Atom() = a.sexp(node) {
                    Some(<$t>::from_be_bytes(a.atom(node).try_into().ok()?))
                } else {
                    None
                }
            }
            fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
                a.new_number((*self).into())
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
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        match a.sexp(node) {
            SExp::Pair(first, rest) => Some((A::from_clvm(a, first)?, B::from_clvm(a, rest)?)),
            SExp::Atom() => None,
        }
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        let first = self.0.to_clvm(a)?;
        let rest = self.1.to_clvm(a)?;
        a.new_pair(first, rest)
    }
}

impl<T> Allocate for Vec<T>
where
    T: Allocate,
{
    fn from_clvm(a: &Allocator, mut node: NodePtr) -> Option<Self> {
        let mut list = Vec::new();
        while let SExp::Pair(first, rest) = a.sexp(node) {
            list.push(T::from_clvm(a, first)?);
            node = rest;
        }
        if nullp(a, node) {
            Some(list)
        } else {
            None
        }
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        let mut list = a.null();
        for item in self.iter().rev() {
            let item_ptr = item.to_clvm(a)?;
            list = a.new_pair(item_ptr, list)?;
        }
        Ok(list)
    }
}

impl<const N: usize> Allocate for [u8; N] {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        if let SExp::Atom() = a.sexp(node) {
            a.atom(node).try_into().ok()
        } else {
            None
        }
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        a.new_atom(self)
    }
}

impl Allocate for Program {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        Program::parse(&mut Cursor::new(&node_to_bytes(a, node).ok()?)).ok()
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        let mut bytes = Vec::new();
        self.stream(&mut bytes)
            .map_err(|error| EvalErr(a.null(), error.to_string()))?;
        node_from_bytes(a, &bytes).map_err(|error| EvalErr(a.null(), error.to_string()))
    }
}

impl Allocate for PublicKey {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        let bytes = <[u8; 48]>::from_clvm(a, node)?;
        PublicKey::validate(&bytes)
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        self.to_bytes().to_clvm(a)
    }
}
