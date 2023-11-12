use crate::{from_clvm, simplify_int_bytes, to_clvm, ClvmValue, FromClvm, FromClvmError, ToClvm};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<Node, const BYTE: u8> ToClvm<Node> for MatchByte<BYTE> {
    to_clvm!(Node, self, f, {
        let bytes = BYTE.to_be_bytes();
        let slice = simplify_int_bytes(&bytes);
        f(ClvmValue::Atom(slice))
    });
}

impl<Node, const BYTE: u8> FromClvm<Node> for MatchByte<BYTE> {
    from_clvm!(Node, f, ptr, {
        match f(ptr) {
            ClvmValue::Atom(&[]) if BYTE == 0 => Ok(Self),
            ClvmValue::Atom(&[byte]) if byte == BYTE && BYTE > 0 => Ok(Self),
            ClvmValue::Atom(..) => Err(FromClvmError::Invalid(format!("expected {BYTE}"))),
            ClvmValue::Pair(..) => Err(FromClvmError::ExpectedAtom),
        }
    });
}

#[cfg(test)]
mod tests {
    use clvmr::Allocator;

    use crate::AllocatorExt;

    use super::*;

    #[test]
    fn test_zero() {
        let a = &mut Allocator::new();
        let atom = a.value_to_ptr(MatchByte::<0>).unwrap();
        a.value_from_ptr::<MatchByte<0>>(atom).unwrap();
    }

    #[test]
    fn test_one() {
        let a = &mut Allocator::new();
        let atom = a.value_to_ptr(MatchByte::<1>).unwrap();
        a.value_from_ptr::<MatchByte<1>>(atom).unwrap();
    }

    #[test]
    fn test_max() {
        let a = &mut Allocator::new();
        let atom = a.value_to_ptr(MatchByte::<255>).unwrap();
        a.value_from_ptr::<MatchByte<255>>(atom).unwrap();
    }
}
