use std::ops::Deref;

use clvm_traits::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwnedAtom(Vec<u8>);

impl OwnedAtom {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl AsRef<[u8]> for OwnedAtom {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for OwnedAtom {
    fn from(data: Vec<u8>) -> Self {
        Self(data)
    }
}

impl From<OwnedAtom> for Vec<u8> {
    fn from(atom: OwnedAtom) -> Self {
        atom.0
    }
}

impl Deref for OwnedAtom {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N> ToClvm<N> for OwnedAtom {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        encoder.encode_atom(self)
    }
}

impl<N> FromClvm<N> for OwnedAtom {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        decoder
            .decode_atom(&node)
            .map(|atom| Self::new(atom.as_ref().to_vec()))
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

        let ptr = OwnedAtom::new(Vec::new()).to_clvm(a).unwrap();
        let bytes = node_to_bytes(a, ptr).unwrap();
        assert_eq!(hex::encode(bytes), "80".to_owned());

        let ptr = OwnedAtom::new(b"hello".to_vec()).to_clvm(a).unwrap();
        let bytes = node_to_bytes(a, ptr).unwrap();
        assert_eq!(hex::encode(bytes), "8568656c6c6f".to_owned());
    }

    #[test]
    fn test_from_clvm() {
        let a = &mut Allocator::new();

        let ptr = node_from_bytes(a, &hex::decode("80").unwrap()).unwrap();
        let value = OwnedAtom::from_clvm(a, ptr).unwrap();
        assert_eq!(value, OwnedAtom::new(Vec::new()));

        let ptr = node_from_bytes(a, &hex::decode("8568656c6c6f").unwrap()).unwrap();
        let value = OwnedAtom::from_clvm(a, ptr).unwrap();
        assert_eq!(value, OwnedAtom::new(b"hello".to_vec()));
    }
}
