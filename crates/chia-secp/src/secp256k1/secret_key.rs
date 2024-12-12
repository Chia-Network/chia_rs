use std::{
    fmt,
    hash::{Hash, Hasher},
};

use k256::ecdsa::{Error, SigningKey};

use super::{Secp256k1PublicKey, Secp256k1Signature};

#[derive(Clone, PartialEq, Eq)]
pub struct Secp256k1SecretKey(SigningKey);

impl Hash for Secp256k1SecretKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state);
    }
}

impl fmt::Debug for Secp256k1SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secp256k1SecretKey(...)")
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Secp256k1SecretKey {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Self::from_bytes(u.arbitrary()?).map_err(|_| arbitrary::Error::IncorrectFormat)
    }
}

impl Secp256k1SecretKey {
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes().into()
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, Error> {
        Ok(Self(SigningKey::from_bytes((&bytes).into())?))
    }

    pub fn public_key(&self) -> Secp256k1PublicKey {
        Secp256k1PublicKey(*self.0.verifying_key())
    }

    pub fn sign_prehashed(&self, message_hash: [u8; 32]) -> Result<Secp256k1Signature, Error> {
        Ok(Secp256k1Signature(
            self.0.sign_prehash_recoverable(&message_hash)?.0,
        ))
    }
}
