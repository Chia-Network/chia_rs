use std::ops::Deref;

use clvm_traits::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizedAtom<const LEN: usize>([u8; LEN]);

impl<const LEN: usize> SizedAtom<LEN> {
    pub fn new(data: [u8; LEN]) -> Self {
        Self(data)
    }

    pub fn as_bytes(self) -> [u8; LEN] {
        self.0
    }
}

impl<const LEN: usize> AsRef<[u8]> for SizedAtom<LEN> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const LEN: usize> Deref for SizedAtom<LEN> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const LEN: usize> From<[u8; LEN]> for SizedAtom<LEN> {
    fn from(data: [u8; LEN]) -> Self {
        Self(data)
    }
}

impl<const LEN: usize> From<SizedAtom<LEN>> for [u8; LEN] {
    fn from(atom: SizedAtom<LEN>) -> Self {
        atom.0
    }
}

impl<N, const LEN: usize> ToClvm<N> for SizedAtom<LEN> {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(self)
    }
}

impl<N, const LEN: usize> FromClvm<N> for SizedAtom<LEN> {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let atom = decoder.decode_atom(&node)?;
        let bytes = atom.as_ref();

        Ok(Self::new(bytes.try_into().map_err(|_| {
            FromClvmError::WrongAtomLength {
                expected: LEN,
                found: bytes.len(),
            }
        })?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use clvmr::{
        serde::{node_from_bytes, node_to_bytes},
        Allocator,
    };

    #[test]
    fn test_to_clvm() {
        let a = &mut Allocator::new();

        let ptr = SizedAtom::new([]).to_clvm(a).unwrap();
        let bytes = node_to_bytes(a, ptr).unwrap();
        assert_eq!(hex::encode(bytes), "80".to_owned());

        let ptr = SizedAtom::new(*b"hello").to_clvm(a).unwrap();
        let bytes = node_to_bytes(a, ptr).unwrap();
        assert_eq!(hex::encode(bytes), "8568656c6c6f".to_owned());
    }

    #[test]
    fn test_from_clvm() {
        let a = &mut Allocator::new();

        let ptr = node_from_bytes(a, &hex::decode("80").unwrap()).unwrap();
        let value = SizedAtom::from_clvm(a, ptr).unwrap();
        assert_eq!(value, SizedAtom::new([]));

        let ptr = node_from_bytes(a, &hex::decode("8568656c6c6f").unwrap()).unwrap();
        let value = SizedAtom::from_clvm(a, ptr).unwrap();
        assert_eq!(value, SizedAtom::new(*b"hello"));
    }
}
