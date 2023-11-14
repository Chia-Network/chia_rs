use crate::{from_clvm, to_clvm, FromClvm, ToClvm};

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
    from_clvm!(Node, _f, ptr, { Ok(Self(ptr)) });
}

impl<Node> ToClvm<Node> for Raw<Node>
where
    Node: Clone,
{
    to_clvm!(Node, self, _f, { Ok(self.0.clone()) });
}
