use crate::secret_key::is_all_zero;
use crate::DerivableKey;
use blst::*;
use chia_traits::chia_error::{Error, Result};
use chia_traits::{read_bytes, Streamable};
use sha2::{digest::FixedOutput, Digest, Sha256};
use std::fmt;
use std::io::Cursor;
use std::mem::MaybeUninit;

#[derive(Clone)]
pub struct PublicKey(pub(crate) blst_p1);

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
            return Ok(Self::default());
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

        let p1 = unsafe {
            let mut p1_affine = MaybeUninit::<blst_p1_affine>::uninit();
            if blst_p1_uncompress(p1_affine.as_mut_ptr(), bytes as *const u8)
                != BLST_ERROR::BLST_SUCCESS
            {
                return Err(Error::Custom("PublicKey is invalid".to_string()));
            }
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_from_affine(p1.as_mut_ptr(), &p1_affine.assume_init());
            p1.assume_init()
        };
        let ret = Self(p1);
        if !ret.is_valid() {
            Err(Error::Custom("PublicKey is invalid".to_string()))
        } else {
            Ok(ret)
        }
    }

    pub fn to_bytes(&self) -> [u8; 48] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; 48]>::uninit();
            blst_p1_compress(bytes.as_mut_ptr() as *mut u8, &self.0);
            bytes.assume_init()
        }
    }

    pub fn is_valid(&self) -> bool {
        // Infinity was considered a valid G1Element in older Relic versions
        // For historical compatibililty this behavior is maintained.
        unsafe { blst_p1_is_inf(&self.0) || blst_p1_in_g1(&self.0) }
    }

    pub fn get_fingerprint(&self) -> u32 {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        let hash: [u8; 32] = hasher.finalize_fixed().into();
        u32::from_be_bytes(hash[0..4].try_into().unwrap())
    }
}

impl PartialEq for PublicKey {
    fn eq(&self, other: &Self) -> bool {
        unsafe { blst_p1_is_equal(&self.0, &other.0) }
    }
}
impl Eq for PublicKey {}

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

impl Default for PublicKey {
    fn default() -> Self {
        unsafe {
            let p1 = MaybeUninit::<blst_p1>::zeroed();
            Self(p1.assume_init())
        }
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self.to_bytes()))
    }
}

impl DerivableKey for PublicKey {
    fn derive_unhardened(&self, idx: u32) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        hasher.update(idx.to_be_bytes());
        let digest: [u8; 32] = hasher.finalize_fixed().into();

        let p1 = unsafe {
            let mut nonce = MaybeUninit::<blst_scalar>::uninit();
            blst_scalar_from_lendian(nonce.as_mut_ptr(), digest.as_ptr());
            let mut bte = MaybeUninit::<[u8; 48]>::uninit();
            blst_bendian_from_scalar(bte.as_mut_ptr() as *mut u8, nonce.as_ptr());
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_mult(
                p1.as_mut_ptr(),
                blst_p1_generator(),
                bte.as_ptr() as *const u8,
                256,
            );
            blst_p1_add(p1.as_mut_ptr(), p1.as_mut_ptr(), &self.0);
            p1.assume_init()
        };
        PublicKey(p1)
    }
}

#[cfg(test)]
use hex::FromHex;

#[cfg(test)]
use crate::SecretKey;

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
    assert_eq!(pk, PublicKey::default());
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

#[test]
fn test_default_is_valid() {
    let pk = PublicKey::default();
    assert!(pk.is_valid());
}

#[test]
fn test_infinity_is_valid() {
    let mut data = [0u8; 48];
    data[0] = 0xc0;
    let pk = PublicKey::from_bytes(&data).unwrap();
    assert!(pk.is_valid());
}

#[test]
fn test_is_valid() {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    for _i in 0..50 {
        rng.fill(data.as_mut_slice());
        let sk = SecretKey::from_seed(&data);
        let pk = sk.public_key();
        assert!(pk.is_valid());
    }
}
