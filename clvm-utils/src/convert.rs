use clvmr::{allocator::NodePtr, Allocator};

use crate::Result;

mod bls;
mod macros;
mod matchers;
mod primitives;

pub use matchers::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LazyNode(pub NodePtr);

pub trait ToClvm: Sized {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr>;
}

pub trait FromClvm: Sized {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self>;
}

impl<T: ToClvm> ToClvm for &T {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        ToClvm::to_clvm(*self, a)
    }
}

impl ToClvm for LazyNode {
    fn to_clvm(&self, _a: &mut Allocator) -> Result<NodePtr> {
        Ok(self.0)
    }
}

impl FromClvm for LazyNode {
    fn from_clvm(_a: &Allocator, node: NodePtr) -> Result<Self> {
        Ok(Self(node))
    }
}
