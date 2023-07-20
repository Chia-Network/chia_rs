use std::array::TryFromSliceError;

use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{Error, FromClvm, Result, ToClvm};

impl<const N: usize> FromClvm for [u8; N] {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom() = a.sexp(node) {
            a.atom(node)
                .try_into()
                .map_err(|error: TryFromSliceError| Error::Reason(error.to_string()))
        } else {
            Err(Error::ExpectedAtom(node))
        }
    }
}

impl<const N: usize> ToClvm for [u8; N] {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        a.new_atom(self).map_err(Error::Allocator)
    }
}
