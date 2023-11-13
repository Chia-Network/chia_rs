use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{ClvmValue, FromClvm, FromClvmError, ToClvm, ToClvmError};

pub trait ToPtr: ToClvm<NodePtr> {
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

pub trait FromPtr: FromClvm<NodePtr>
where
    Self: Sized,
{
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
