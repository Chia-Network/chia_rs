use clvmr::{allocator::SExp, Allocator, NodePtr};

use crate::{FromClvm, FromClvmError};

pub trait ClvmDecoder {
    type Node: Clone;

    fn decode_atom(&self, node: &Self::Node) -> Result<&[u8], FromClvmError>;
    fn decode_pair(&self, node: &Self::Node) -> Result<(Self::Node, Self::Node), FromClvmError>;

    /// This is a helper function that just calls `clone` on the node.
    /// It's required only because the compiler can't infer that `N` is `Clone`,
    /// since there's no `Clone` bound on the `FromClvm` trait.
    fn clone_node(&self, node: &Self::Node) -> Self::Node {
        node.clone()
    }
}

impl ClvmDecoder for Allocator {
    type Node = NodePtr;

    fn decode_atom(&self, node: &Self::Node) -> Result<&[u8], FromClvmError> {
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
}

pub trait FromNodePtr {
    fn from_node_ptr(a: &Allocator, node: NodePtr) -> Result<Self, FromClvmError>
    where
        Self: Sized;
}

impl<T> FromNodePtr for T
where
    T: FromClvm<NodePtr>,
{
    fn from_node_ptr(a: &Allocator, node: NodePtr) -> Result<Self, FromClvmError>
    where
        Self: Sized,
    {
        T::from_clvm(a, node)
    }
}

impl FromClvm<NodePtr> for NodePtr {
    fn from_clvm(
        _decoder: &impl ClvmDecoder<Node = NodePtr>,
        node: NodePtr,
    ) -> Result<Self, FromClvmError> {
        Ok(node)
    }
}
