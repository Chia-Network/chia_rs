use std::fmt;
use std::hash::{Hash, Hasher};

use chia_sha2::Sha256;
use p256::ecdsa::signature::hazmat::PrehashVerifier;
use p256::ecdsa::{Error, VerifyingKey};

use super::R1Signature;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct R1PublicKey(pub(crate) VerifyingKey);

impl Hash for R1PublicKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bytes().hash(state);
    }
}

impl fmt::Debug for R1PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "R1PublicKey({self})")
    }
}

impl fmt::Display for R1PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.to_bytes()))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for R1PublicKey {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Self::from_bytes(&u.arbitrary()?).map_err(|_| arbitrary::Error::IncorrectFormat)
    }
}

impl R1PublicKey {
    pub const SIZE: usize = 33;

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0.to_encoded_point(true).as_ref().try_into().unwrap()
    }

    pub fn from_bytes(bytes: &[u8; Self::SIZE]) -> Result<Self, Error> {
        Ok(Self(VerifyingKey::from_sec1_bytes(bytes)?))
    }

    pub fn verify_prehashed(&self, message_hash: &[u8; 32], signature: &R1Signature) -> bool {
        self.0.verify_prehash(message_hash, &signature.0).is_ok()
    }

    pub fn fingerprint(&self) -> u32 {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        let hash = hasher.finalize();
        u32::from_be_bytes(hash[0..4].try_into().unwrap())
    }
}
