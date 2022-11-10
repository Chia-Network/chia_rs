use crate::derivable_key::DerivableKey;
use bls12_381_plus::{G1Affine, G1Projective, Scalar};
use group::Curve;
use num_bigint::BigUint;
use sha2::{Digest, Sha256};

#[derive(PartialEq, Eq, Debug)]
pub struct PublicKey(pub G1Projective);

impl PublicKey {
    pub fn from_bytes(bytes: &[u8; 48]) -> Option<PublicKey> {
        G1Affine::from_compressed(bytes)
            .map(|p| Self(G1Projective::from(&p)))
            .into()
    }

    pub fn to_bytes(&self) -> [u8; 48] {
        self.0.to_affine().to_compressed()
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_identity().unwrap_u8() == 0 && self.0.is_on_curve().unwrap_u8() == 1
    }
}

impl DerivableKey for PublicKey {
    fn derive_unhardened(&self, idx: u32) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        hasher.update(idx.to_be_bytes());
        let digest = hasher.finalize();

        // in an ideal world, we would not need to reach for the sledge-hammer of
        // num-bigint here. This would most likely be faster if implemented in
        // Scalar directly.

        // interpret the hash as an unsigned big-endian number
        let mut nounce = BigUint::from_bytes_be(digest.as_slice());

        let q = BigUint::from_bytes_be(&[
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
        ]);

        // mod by G1 Order
        nounce %= q;

        let raw_bytes = nounce.to_bytes_be();
        let mut bytes = [0_u8; 32];
        bytes[32 - raw_bytes.len()..].copy_from_slice(&raw_bytes);
        bytes.reverse();

        let nounce = Scalar::from_bytes(&bytes).unwrap();

        PublicKey(self.0 + G1Projective::generator() * nounce)
    }
}

#[cfg(test)]
use hex::FromHex;

#[cfg(test)]
use crate::secret_key::SecretKey;

#[test]
fn test_derive_unhardened() {
    let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
    let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
    let pk = sk.public_key();

    // make sure deriving the secret keys produce the same public keys as
    // deriving the public key
    for idx in 0..4_usize {
        let derived_sk = sk.derive_unhardened(idx as u32);
        let derived_pk = pk.derive_unhardened(idx as u32);
        assert_eq!(derived_pk.to_bytes(), derived_sk.public_key().to_bytes());
    }
}

#[cfg(test)]
use rand::{Rng, SeedableRng};

#[cfg(test)]
use rand::rngs::StdRng;

#[test]
fn test_from_bytes() {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 48];
    for _i in 0..50 {
        rng.fill(data.as_mut_slice());
        // just any random bytes are not a valid key and should fail
        assert_eq!(PublicKey::from_bytes(&data), None);
    }
}

#[test]
fn test_roundtrip() {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    for _i in 0..50 {
        rng.fill(data.as_mut_slice());
        let sk = SecretKey::from_seed(&data);
        let pk = sk.public_key();
        let bytes = pk.to_bytes();
        let pk2 = PublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(pk, pk2);
    }
}

// ERROR test is_valid
