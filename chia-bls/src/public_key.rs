use crate::derivable_key::DerivableKey;
use crate::secret_key::is_all_zero;
use bls12_381_plus::{G1Affine, G1Projective, Scalar};
use chia_traits::chia_error::{Error, Result};
use chia_traits::{read_bytes, Streamable};
use group::Curve;
use num_bigint::BigUint;
use sha2::{digest::FixedOutput, Digest, Sha256};
use std::io::Cursor;

#[derive(PartialEq, Eq, Debug)]
pub struct PublicKey(pub G1Projective);

impl PublicKey {
    pub fn from_bytes(bytes: &[u8; 48]) -> Result<Self> {
        // check if the element is canonical
        // the first 3 bits have special meaning
        let zeros_only = is_all_zero(&bytes[1..]);

        if (bytes[0] & 0xc0) == 0xc0 {
            // enforce that infinity must be 0xc0000..00
            if bytes[0] != 0xc0 || !zeros_only {
                return Err(Error::Custom(
                    "Given G1 infinity element must be canonical".to_string(),
                ));
            }
            // return infinity element (point all zero)
            return Ok(Self(G1Projective::identity()));
        } else {
            if (bytes[0] & 0xc0) != 0x80 {
                return Err(Error::Custom(
                    "Given G1 non-infinity element must start with 0b10".to_string(),
                ));
            }
            if zeros_only {
                return Err(Error::Custom(
                    "G1 non-infinity element can't have only zeros".to_string(),
                ));
            }
        }

        match G1Affine::from_compressed(bytes).into() {
            Some(p) => Ok(Self(G1Projective::from(&p))),
            None => Err(Error::Custom("PublicKey is invalid".to_string())),
        }
    }

    pub fn to_bytes(&self) -> [u8; 48] {
        self.0.to_affine().to_compressed()
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_identity().unwrap_u8() == 0 && self.0.is_on_curve().unwrap_u8() == 1
    }

    pub fn get_fingerprint(&self) -> u32 {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        let hash: [u8; 32] = hasher.finalize_fixed().into();
        u32::from_be_bytes(hash[0..4].try_into().unwrap())
    }
}

impl Streamable for PublicKey {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.to_bytes());
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(&self.to_bytes());
        Ok(())
    }

    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Self::from_bytes(read_bytes(input, 48)?.try_into().unwrap())
    }
}

impl DerivableKey for PublicKey {
    fn derive_unhardened(&self, idx: u32) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        hasher.update(idx.to_be_bytes());
        let digest: [u8; 32] = hasher.finalize_fixed().into();

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

#[cfg(test)]
use rstest::rstest;

#[test]
fn test_from_bytes() {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 48];
    for _i in 0..50 {
        rng.fill(data.as_mut_slice());
        // clear the bits that mean infinity
        data[0] = 0x80;
        // just any random bytes are not a valid key and should fail
        assert_eq!(
            PublicKey::from_bytes(&data).unwrap_err(),
            Error::Custom("PublicKey is invalid".to_string())
        );
    }
}

#[cfg(test)]
#[rstest]
#[case("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001", "Given G1 infinity element must be canonical")]
#[case("c08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "Given G1 infinity element must be canonical")]
#[case("c80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "Given G1 infinity element must be canonical")]
#[case("e00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "Given G1 infinity element must be canonical")]
#[case("d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "Given G1 infinity element must be canonical")]
#[case("800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "G1 non-infinity element can't have only zeros")]
#[case("400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", "Given G1 non-infinity element must start with 0b10")]
fn test_from_bytes_failures(#[case] input: &str, #[case()] error: &str) {
    let bytes: [u8; 48] = hex::decode(input).unwrap().try_into().unwrap();
    assert_eq!(
        PublicKey::from_bytes(&bytes).unwrap_err(),
        Error::Custom(error.to_string())
    );
}

#[test]
fn test_from_bytes_infinity() {
    let bytes: [u8; 48] = hex::decode("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap().try_into().unwrap();
    let pk = PublicKey::from_bytes(&bytes).unwrap();
    assert!(pk.0.is_identity().unwrap_u8() != 0);
}

#[test]
fn test_get_fingerprint() {
    let bytes: [u8; 48] = hex::decode("997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2")
        .unwrap()
        .as_slice()
        .try_into()
        .unwrap();
    let pk = PublicKey::from_bytes(&bytes).unwrap();
    assert_eq!(pk.get_fingerprint(), 651010559);
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
