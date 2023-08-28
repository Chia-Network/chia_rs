use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{ClvmTree, Error, FromClvm, Result, Value};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<N, const BYTE: u8> ClvmTree<N> for MatchByte<BYTE> {
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        BYTE.collect_tree(f)
    }
}

impl<const BYTE: u8> FromClvm for MatchByte<BYTE> {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom = a.sexp(node) {
            match a.atom(node) {
                [] if BYTE == 0 => Ok(Self),
                [byte] if *byte == BYTE && BYTE > 0 => Ok(Self),
                _ => Err(Error::msg(format!(
                    "expected an atom with a value of {}",
                    BYTE
                ))),
            }
        } else {
            Err(Error::msg("expected atom"))
        }
    }
}
