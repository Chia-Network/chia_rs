use crate::{simplify_int_bytes, ClvmValue, FromClvm, FromClvmError, ToClvm, ToClvmError};

/// A simple type for performing validation on an atom,
/// ensuring that it matches a given byte value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MatchByte<const BYTE: u8>;

impl<Node, const BYTE: u8> ToClvm<Node> for MatchByte<BYTE>
where
    Node: Clone,
{
    fn to_clvm(
        &self,
        f: &mut impl FnMut(ClvmValue<Node>) -> Result<Node, ToClvmError>,
    ) -> Result<Node, ToClvmError> {
        let bytes = BYTE.to_be_bytes();
        let slice = simplify_int_bytes(&bytes);
        f(ClvmValue::Atom(slice))
    }
}

impl<Node, const BYTE: u8> FromClvm<Node> for MatchByte<BYTE>
where
    Node: Clone,
{
    fn from_clvm<'a>(
        f: &mut impl FnMut(&Node) -> ClvmValue<'a, Node>,
        ptr: Node,
    ) -> Result<Self, FromClvmError> {
        match f(&ptr) {
            ClvmValue::Atom(&[]) if BYTE == 0 => Ok(Self),
            ClvmValue::Atom(&[byte]) if byte == BYTE && BYTE > 0 => Ok(Self),
            ClvmValue::Atom(..) => Err(FromClvmError::Invalid(format!("expected {BYTE}"))),
            ClvmValue::Pair(..) => Err(FromClvmError::ExpectedAtom),
        }
    }
}

#[cfg(test)]
mod tests {
    use clvmr::Allocator;

    use crate::{FromPtr, ToPtr};

    use super::*;

    #[test]
    fn test_zero() {
        let a = &mut Allocator::new();
        let atom = MatchByte::<0>.to_ptr(a).unwrap();
        <MatchByte<0>>::from_ptr(a, atom).unwrap();
    }

    #[test]
    fn test_one() {
        let a = &mut Allocator::new();
        let atom = MatchByte::<1>.to_ptr(a).unwrap();
        <MatchByte<1>>::from_ptr(a, atom).unwrap();
    }

    #[test]
    fn test_max() {
        let a = &mut Allocator::new();
        let atom = MatchByte::<255>.to_ptr(a).unwrap();
        <MatchByte<255>>::from_ptr(a, atom).unwrap();
    }
}
