use crate::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

/// A wrapper for an intermediate CLVM value. This is required to
/// implement `ToClvm` and `FromClvm` for `N`, since the compiler
/// cannot guarantee that the generic `N` type doesn't already
/// implement these traits.
pub struct Raw<N>(pub N);

impl<N, D: ClvmDecoder<Node = N>> FromClvm<D> for Raw<N> {
    fn from_clvm(_decoder: &D, node: N) -> Result<Self, FromClvmError> {
        Ok(Self(node))
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for Raw<N> {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        Ok(encoder.clone_node(&self.0))
    }
}
