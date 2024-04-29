use std::ops::Deref;

use crate::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

/// A wrapper for an intermediate CLVM value. This is required to
/// implement `ToClvm` and `FromClvm` for `N`, since the compiler
/// cannot guarantee that the generic `N` type doesn't already
/// implement these traits.
pub struct Raw<N>(pub N);

impl<N> FromClvm<N> for Raw<N> {
    fn from_clvm(_decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        Ok(Self(node))
    }
}

impl<N> ToClvm<N> for Raw<N> {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        Ok(encoder.clone_node(&self.0))
    }
}

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
