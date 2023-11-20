use clvmr::{allocator::NodePtr, Allocator};

use crate::ToClvmError;

pub trait ClvmEncoder {
    type Node: Clone;

    fn encode_atom(&mut self, bytes: &[u8]) -> Result<Self::Node, ToClvmError>;
    fn encode_pair(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, ToClvmError>;

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
