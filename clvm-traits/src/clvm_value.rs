/// The simplest representation of a CLVM value.
/// This is used for building trees with the `ToClvm` and `FromClvm` traits.
pub enum ClvmValue<'a, Node> {
    /// An atomic value in CLVM, represented with bytes.
    Atom(&'a [u8]),

    /// A cons-pair value in CLVM, represented with two `Node` values.
    /// Nodes are intermediate results of conversions, such as `NodePtr` when working with a CLVM `Allocator`.
    Pair(Node, Node),
}
