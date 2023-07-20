use clvmr::{allocator::NodePtr, Allocator};

use crate::Result;

mod arrays;
mod bls;
mod macros;
mod matchers;
mod primitives;

pub use matchers::*;

pub trait ToClvm: Sized {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr>;
}

pub trait FromClvm: Sized {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self>;
}
