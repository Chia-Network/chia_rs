pub enum ClvmValue<'a, Node> {
    Atom(&'a [u8]),
    Pair(Node, Node),
}
