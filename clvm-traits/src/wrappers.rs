use crate::{ClvmValue, FromClvm, FromClvmError, ToClvm, ToClvmError};

/// A wrapper for an intermediate CLVM value. This is required to
/// implement `ToClvm` and `FromClvm` for `Node`, since the compiler
/// cannot guarantee that the generic `Node` type doesn't already
/// implement these traits.
pub struct Raw<Node>(pub Node)
where
    Node: Clone;

impl<Node> FromClvm<Node> for Raw<Node>
where
    Node: Clone,
{
    fn from_clvm<'a>(
        _f: &mut impl FnMut(&Node) -> ClvmValue<'a, Node>,
        ptr: Node,
    ) -> Result<Self, FromClvmError> {
        Ok(Self(ptr))
    }
}

impl<Node> ToClvm<Node> for Raw<Node>
where
    Node: Clone,
{
    fn to_clvm(
        &self,
        _f: &mut impl FnMut(ClvmValue<Node>) -> Result<Node, ToClvmError>,
    ) -> Result<Node, ToClvmError> {
        Ok(self.0.clone())
    }
}
