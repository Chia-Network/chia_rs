use crate::{from_clvm, to_clvm, FromClvm, ToClvm};

pub struct Raw<T>(pub T);

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
