#[derive(Debug)]
pub enum Value<'a, T> {
    Atom(&'a [u8]),
    Pair(T, T),
}
