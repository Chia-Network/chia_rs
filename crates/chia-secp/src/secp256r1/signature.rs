use std::{
    fmt,
    hash::{Hash, Hasher},
};

use p256::ecdsa::{Error, Signature};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Secp256r1Signature(pub(crate) Signature);

impl Hash for Secp256r1Signature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state);
    }
}

impl fmt::Debug for Secp256r1Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secp256r1Signature({self})")
    }
}

impl fmt::Display for Secp256r1Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.to_bytes()))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Secp256r1Signature {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Self::from_bytes(u.arbitrary()?).map_err(|_| arbitrary::Error::IncorrectFormat)
    }
}

impl Secp256r1Signature {
    pub const SIZE: usize = 64;

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0.to_bytes().into()
    }

    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> Result<Self, Error> {
        Ok(Self(Signature::from_slice(&bytes)?))
    }
}
