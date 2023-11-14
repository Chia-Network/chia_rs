use clvmr::{
    allocator::{NodePtr, SExp},
    sha2::{Digest, Sha256},
    Allocator,
};

use crate::{ClvmValue, FromClvm, FromClvmError, ToClvm, ToClvmError};

/// A wrapper trait for `ToClvm<NodePtr>` that implements conversion to a `NodePtr`.
pub trait ToPtr: ToClvm<NodePtr> {
    /// Allocates the value on a given CLVM allocator, returning the corresponding `NodePtr`.
    fn to_ptr(&self, a: &mut Allocator) -> Result<NodePtr, ToClvmError>;
}

impl<T> ToPtr for T
where
    T: ToClvm<NodePtr>,
{
    fn to_ptr(&self, a: &mut Allocator) -> Result<NodePtr, ToClvmError> {
        self.to_clvm(&mut |value| match value {
            ClvmValue::Atom(bytes) => match bytes {
                [] => Ok(a.null()),
                [1] => Ok(a.one()),
                _ => Ok(a.new_atom(bytes).or(Err(ToClvmError::LimitReached))?),
            },
            ClvmValue::Pair(first, rest) => {
                Ok(a.new_pair(first, rest).or(Err(ToClvmError::LimitReached))?)
            }
        })
    }
}

/// A wrapper trait for `FromClvm<NodePtr>` that implements conversion from a `NodePtr`.
pub trait FromPtr: FromClvm<NodePtr>
where
    Self: Sized,
{
    /// Reconstructs a value from a given CLVM allocator and `NodePtr`.
    fn from_ptr(a: &Allocator, ptr: NodePtr) -> Result<Self, FromClvmError>;
}

impl<T> FromPtr for T
where
    T: FromClvm<NodePtr>,
{
    fn from_ptr(a: &Allocator, ptr: NodePtr) -> Result<Self, FromClvmError> {
        Self::from_clvm(
            &mut |ptr| match a.sexp(*ptr) {
                SExp::Atom => ClvmValue::Atom(a.atom(*ptr)),
                SExp::Pair(first, rest) => ClvmValue::Pair(first, rest),
            },
            ptr,
        )
    }
}

/// A wrapper around a `[u8; 32]` value, which represents the tree hash of a program.
/// This is used since `[T; N]` is serialized as a list of `T` values, rather than bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeHash(pub [u8; 32]);

/// A wrapper trait for `ToClvm<TreeHash>` that implements conversion to a `TreeHash`.
pub trait ToTreeHash: ToClvm<TreeHash> {
    /// Computes the tree hash of the value.
    fn tree_hash(&self) -> Result<TreeHash, ToClvmError>;
}

impl<T> ToTreeHash for T
where
    T: ToClvm<TreeHash>,
{
    fn tree_hash(&self) -> Result<TreeHash, ToClvmError> {
        self.to_clvm(&mut |value| match value {
            ClvmValue::Atom(bytes) => {
                let mut sha256 = Sha256::new();
                sha256.update([1]);
                sha256.update(bytes);
                Ok(TreeHash(sha256.finalize().try_into().unwrap()))
            }
            ClvmValue::Pair(first, rest) => {
                let mut sha256 = Sha256::new();
                sha256.update([2]);
                sha256.update(first.0);
                sha256.update(rest.0);
                Ok(TreeHash(sha256.finalize().try_into().unwrap()))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_hash() {
        let value = (42, ([1, 2, 3], "Hello, world!"));
        let ptr = value.tree_hash().unwrap().0;
        assert_eq!(
            hex::encode(ptr),
            "7c3670f319e07cff6d433e4c22e0895f1f0a10bad5bbcd23c32e3bc5589c23cb"
        );
    }
}
