use clvmr::{
    allocator::{NodePtr, SExp},
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
