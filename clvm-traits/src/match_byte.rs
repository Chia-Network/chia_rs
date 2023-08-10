use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{Error, FromClvm, Result, ToClvm};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<const BYTE: u8> ToClvm for MatchByte<BYTE> {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        a.new_number(BYTE.into()).map_err(Error::Allocator)
    }
}

impl<const BYTE: u8> FromClvm for MatchByte<BYTE> {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom() = a.sexp(node) {
            match a.atom(node) {
                [] if BYTE == 0 => Ok(Self),
                [byte] if *byte == BYTE && BYTE > 0 => Ok(Self),
                _ => Err(Error::Custom(format!(
                    "expected an atom with a value of {}",
                    BYTE
                ))),
            }
        } else {
            Err(Error::ExpectedAtom(node))
        }
    }
}
