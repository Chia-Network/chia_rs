use std::io::Cursor;

use chia_protocol::{BytesImpl, Coin, Program};
use chia_traits::Streamable;
use clvmr::{
    allocator::{NodePtr, SExp},
    serde::{node_from_bytes, node_to_bytes},
    Allocator,
};

use crate::{clvm_list, match_list, Error, FromClvm, Result, ToClvm};

impl FromClvm for Program {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        let bytes = node_to_bytes(a, node).map_err(|error| Error::Reason(error.to_string()))?;
        Self::parse(&mut Cursor::new(&bytes)).map_err(|error| Error::Reason(error.to_string()))
    }
}

impl ToClvm for Program {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        node_from_bytes(a, self.as_ref()).map_err(|error| Error::Reason(error.to_string()))
    }
}

impl<const N: usize> FromClvm for BytesImpl<N> {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom() = a.sexp(node) {
            Self::parse(&mut Cursor::new(a.atom(node)))
                .map_err(|error| Error::Reason(error.to_string()))
        } else {
            Err(Error::ExpectedAtom(node))
        }
    }
}

impl<const N: usize> ToClvm for BytesImpl<N> {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let bytes: &[u8; N] = self.into();
        bytes.to_clvm(a)
    }
}

impl FromClvm for Coin {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        let value = <match_list!(BytesImpl<32>, BytesImpl<32>, u64)>::from_clvm(a, node)?;
        Ok(Coin {
            parent_coin_info: value.0,
            puzzle_hash: value.1 .0,
            amount: value.1 .1 .0,
        })
    }
}

impl ToClvm for Coin {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        clvm_list!(self.parent_coin_info, self.puzzle_hash, self.amount).to_clvm(a)
    }
}
