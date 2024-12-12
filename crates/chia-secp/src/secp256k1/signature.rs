use std::{
    fmt,
    hash::{Hash, Hasher},
};

use k256::ecdsa::{Error, Signature};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Secp256k1Signature(pub(crate) Signature);

impl Hash for Secp256k1Signature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state);
    }
}

impl fmt::Debug for Secp256k1Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secp256k1Signature({self})")
    }
}

impl fmt::Display for Secp256k1Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.to_bytes()))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Secp256k1Signature {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Self::from_bytes(u.arbitrary()?).map_err(|_| arbitrary::Error::IncorrectFormat)
    }
}

impl Secp256k1Signature {
    pub const SIZE: usize = 64;

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0.to_bytes().into()
    }

    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> Result<Self, Error> {
        Ok(Self(Signature::from_slice(&bytes)?))
    }
}
