use clvmr::{allocator::SExp, Allocator, Atom, NodePtr};
use num_bigint::BigInt;

use crate::{
    destructure_list, destructure_quote, match_list, match_quote, FromClvm, FromClvmError,
    MatchByte,
};

pub trait ClvmDecoder: Sized {
    type Node: Clone + FromClvm<Self>;

    fn decode_atom(&self, node: &Self::Node) -> Result<Atom<'_>, FromClvmError>;
    fn decode_pair(&self, node: &Self::Node) -> Result<(Self::Node, Self::Node), FromClvmError>;

    fn decode_bigint(&self, node: &Self::Node) -> Result<BigInt, FromClvmError> {
        let atom = self.decode_atom(node)?;
        Ok(BigInt::from_signed_bytes_be(atom.as_ref()))
    }

    fn decode_curried_arg(
        &self,
        node: &Self::Node,
    ) -> Result<(Self::Node, Self::Node), FromClvmError> {
        let destructure_list!(_, destructure_quote!(first), rest) =
            <match_list!(MatchByte<4>, match_quote!(Self::Node), Self::Node)>::from_clvm(
                self,
                node.clone(),
            )?;
        Ok((first, rest))
    }

    /// This is a helper function that just calls `clone` on the node.
    /// It's required only because the compiler can't infer that `N` is `Clone`,
    /// since there's no `Clone` bound on the `FromClvm` trait.
    fn clone_node(&self, node: &Self::Node) -> Self::Node {
        node.clone()
    }
}

impl ClvmDecoder for Allocator {
    type Node = NodePtr;

    fn decode_atom(&self, node: &Self::Node) -> Result<Atom<'_>, FromClvmError> {
        if let SExp::Atom = self.sexp(*node) {
            Ok(self.atom(*node))
        } else {
            Err(FromClvmError::ExpectedAtom)
        }
    }

    fn decode_pair(&self, node: &Self::Node) -> Result<(Self::Node, Self::Node), FromClvmError> {
        if let SExp::Pair(first, rest) = self.sexp(*node) {
            Ok((first, rest))
        } else {
            Err(FromClvmError::ExpectedPair)
        }
    }

    fn decode_bigint(&self, node: &Self::Node) -> Result<BigInt, FromClvmError> {
        if let SExp::Atom = self.sexp(*node) {
            Ok(self.number(*node))
        } else {
            Err(FromClvmError::ExpectedAtom)
        }
    }
}

impl FromClvm<Allocator> for NodePtr {
    fn from_clvm(_decoder: &Allocator, node: NodePtr) -> Result<Self, FromClvmError> {
        Ok(node)
    }
}
