use crate::{BuildTree, Error, ParseTree, Result, Value};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<N, const BYTE: u8> BuildTree<N> for MatchByte<BYTE> {
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        BYTE.build_tree(f)
    }
}

impl<N, const BYTE: u8> ParseTree<N> for MatchByte<BYTE> {
    fn parse_tree<'a>(f: &impl Fn(N) -> Value<'a, N>, ptr: N) -> Result<Self> {
        match f(ptr) {
            Value::Atom(&[]) if BYTE == 0 => Ok(Self),
            Value::Atom(&[byte]) if byte == BYTE && BYTE > 0 => Ok(Self),
            Value::Atom(_) => Err(Error::msg(format!(
                "expected an atom with a value of {}",
                BYTE
            ))),
            _ => Err(Error::msg("expected atom")),
        }
    }
}
