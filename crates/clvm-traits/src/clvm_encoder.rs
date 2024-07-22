use clvmr::{Allocator, NodePtr};

use crate::{clvm_list, clvm_quote, ToClvm, ToClvmError};

pub trait ClvmEncoder: Sized {
    type Node: Clone + ToClvm<Self::Node>;

    fn encode_atom(&mut self, bytes: &[u8]) -> Result<Self::Node, ToClvmError>;
    fn encode_pair(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, ToClvmError>;

    fn encode_curried_arg(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, ToClvmError> {
        const OP_C: u8 = 4;
        clvm_list!(OP_C, clvm_quote!(first), rest).to_clvm(self)
    }

    /// This is a helper function that just calls `clone` on the node.
    /// It's required only because the compiler can't infer that `N` is `Clone`,
    /// since there's no `Clone` bound on the `ToClvm` trait.
    fn clone_node(&self, node: &Self::Node) -> Self::Node {
        node.clone()
    }
}

impl ClvmEncoder for Allocator {
    type Node = NodePtr;

    fn encode_atom(&mut self, bytes: &[u8]) -> Result<Self::Node, ToClvmError> {
        self.new_atom(bytes).or(Err(ToClvmError::OutOfMemory))
    }

    fn encode_pair(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, ToClvmError> {
        self.new_pair(first, rest).or(Err(ToClvmError::OutOfMemory))
    }
}

pub trait ToNodePtr {
    fn to_node_ptr(&self, a: &mut Allocator) -> Result<NodePtr, ToClvmError>;
}

impl<T> ToNodePtr for T
where
    T: ToClvm<NodePtr>,
{
    fn to_node_ptr(&self, a: &mut Allocator) -> Result<NodePtr, ToClvmError> {
        self.to_clvm(a)
    }
}

impl ToClvm<NodePtr> for NodePtr {
    fn to_clvm(
        &self,
        _encoder: &mut impl ClvmEncoder<Node = NodePtr>,
    ) -> Result<NodePtr, ToClvmError> {
        Ok(*self)
    }
}
