use clvmr::{Allocator, Atom, NodePtr};
use num_bigint::BigInt;

use crate::{clvm_list, clvm_quote, ToClvm, ToClvmError};

pub trait ClvmEncoder: Sized {
    type Node: Clone + ToClvm<Self>;

    fn encode_atom(&mut self, atom: Atom<'_>) -> Result<Self::Node, ToClvmError>;
    fn encode_pair(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, ToClvmError>;

    fn encode_bigint(&mut self, number: BigInt) -> Result<Self::Node, ToClvmError> {
        let bytes = number.to_signed_bytes_be();
        let mut slice = bytes.as_slice();

        // Remove leading zeros.
        while !slice.is_empty() && slice[0] == 0 {
            if slice.len() > 1 && (slice[1] & 0x80 == 0x80) {
                break;
            }
            slice = &slice[1..];
        }

        self.encode_atom(Atom::Borrowed(slice))
    }

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

    fn encode_atom(&mut self, atom: Atom<'_>) -> Result<Self::Node, ToClvmError> {
        match atom {
            Atom::Borrowed(bytes) => self.new_atom(bytes),
            Atom::U32(bytes, _len) => self.new_small_number(u32::from_be_bytes(bytes)),
        }
        .or(Err(ToClvmError::OutOfMemory))
    }

    fn encode_pair(
        &mut self,
        first: Self::Node,
        rest: Self::Node,
    ) -> Result<Self::Node, ToClvmError> {
        self.new_pair(first, rest).or(Err(ToClvmError::OutOfMemory))
    }

    fn encode_bigint(&mut self, number: BigInt) -> Result<Self::Node, ToClvmError> {
        self.new_number(number).or(Err(ToClvmError::OutOfMemory))
    }
}

impl ToClvm<Allocator> for NodePtr {
    fn to_clvm(&self, _encoder: &mut Allocator) -> Result<NodePtr, ToClvmError> {
        Ok(*self)
    }
}
