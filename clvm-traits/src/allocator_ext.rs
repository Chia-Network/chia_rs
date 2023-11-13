use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{ClvmValue, FromClvm, FromClvmError, ToClvm, ToClvmError};

pub trait AllocatorExt {
    fn value_to_ptr(&mut self, value: impl ToClvm<NodePtr>) -> Result<NodePtr, ToClvmError>;
    fn value_from_ptr<T>(&self, ptr: NodePtr) -> Result<T, FromClvmError>
    where
        T: FromClvm<NodePtr>;
}

impl AllocatorExt for Allocator {
    fn value_to_ptr(&mut self, value: impl ToClvm<NodePtr>) -> Result<NodePtr, ToClvmError> {
        value.to_clvm(&mut |value| match value {
            ClvmValue::Atom(bytes) => match bytes {
                [] => Ok(self.null()),
                [1] => Ok(self.one()),
                _ => Ok(self.new_atom(bytes).or(Err(ToClvmError::LimitReached))?),
            },
            ClvmValue::Pair(first, rest) => Ok(self
                .new_pair(first, rest)
                .or(Err(ToClvmError::LimitReached))?),
        })
    }

    fn value_from_ptr<T>(&self, ptr: NodePtr) -> Result<T, FromClvmError>
    where
        T: FromClvm<NodePtr>,
    {
        T::from_clvm(
            &mut |ptr| match self.sexp(*ptr) {
                SExp::Atom => ClvmValue::Atom(self.atom(*ptr)),
                SExp::Pair(first, rest) => ClvmValue::Pair(first, rest),
            },
            ptr,
        )
    }
}
