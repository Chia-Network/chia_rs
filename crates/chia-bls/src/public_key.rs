use crate::secret_key::is_all_zero;
use crate::{DerivableKey, Error, Result};

use blst::*;
use chia_sha2::Sha256;
use chia_traits::{read_bytes, Streamable};
#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyNotImplementedError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign, Neg, SubAssign};

#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(name = "G1Element"),
    derive(chia_py_streamable_macro::PyStreamable)
)]
#[derive(Clone, Copy, Default)]
pub struct PublicKey(pub(crate) blst_p1);

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for PublicKey {
    fn arbitrary(_u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // placeholder
        Ok(Self::default())
    }
}

impl PublicKey {
    pub fn from_bytes_unchecked(bytes: &[u8; 48]) -> Result<Self> {
        // check if the element is canonical
        // the first 3 bits have special meaning
        let zeros_only = is_all_zero(&bytes[1..]);

        if (bytes[0] & 0xc0) == 0xc0 {
            // enforce that infinity must be 0xc0000..00
            if bytes[0] != 0xc0 || !zeros_only {
                return Err(Error::G1NotCanonical);
            }
            // return infinity element (point all zero)
            return Ok(Self::default());
        }

        if (bytes[0] & 0xc0) != 0x80 {
            return Err(Error::G1InfinityInvalidBits);
        }
        if zeros_only {
            return Err(Error::G1InfinityNotZero);
        }

        let p1 = unsafe {
            let mut p1_affine = MaybeUninit::<blst_p1_affine>::uninit();
            let ret = blst_p1_uncompress(p1_affine.as_mut_ptr(), bytes.as_ptr());
            if ret != BLST_ERROR::BLST_SUCCESS {
                return Err(Error::InvalidPublicKey(ret));
            }
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_from_affine(p1.as_mut_ptr(), &p1_affine.assume_init());
            p1.assume_init()
        };
        Ok(Self(p1))
    }

    pub fn generator() -> Self {
        let p1 = unsafe { *blst_p1_generator() };
        Self(p1)
    }

    // Creates a G1 point by multiplying the generator by the specified scalar.
    // This is the same as creating a private key from the scalar, and then get
    // the corresponding public key
    pub fn from_integer(int_bytes: &[u8]) -> Self {
        let p1 = unsafe {
            let mut scalar = MaybeUninit::<blst_scalar>::uninit();
            blst_scalar_from_be_bytes(scalar.as_mut_ptr(), int_bytes.as_ptr(), int_bytes.len());
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_mult(
                p1.as_mut_ptr(),
                blst_p1_generator(),
                scalar.as_ptr().cast::<u8>(),
                256,
            );
            p1.assume_init()
        };
        Self(p1)
    }

    pub fn from_bytes(bytes: &[u8; 48]) -> Result<Self> {
        let ret = Self::from_bytes_unchecked(bytes)?;
        if ret.is_valid() {
            Ok(ret)
        } else {
            Err(Error::InvalidPublicKey(BLST_ERROR::BLST_POINT_NOT_ON_CURVE))
        }
    }

    pub fn from_uncompressed(buf: &[u8; 96]) -> Result<Self> {
        let p1 = unsafe {
            let mut p1_affine = MaybeUninit::<blst_p1_affine>::uninit();
            let ret = blst_p1_deserialize(p1_affine.as_mut_ptr(), buf.as_ptr());
            if ret != BLST_ERROR::BLST_SUCCESS {
                return Err(Error::InvalidSignature(ret));
            }
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_from_affine(p1.as_mut_ptr(), &p1_affine.assume_init());
            p1.assume_init()
        };
        Ok(Self(p1))
    }

    pub fn to_bytes(&self) -> [u8; 48] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; 48]>::uninit();
            blst_p1_compress(bytes.as_mut_ptr().cast::<u8>(), &self.0);
            bytes.assume_init()
        }
    }

    pub fn is_valid(&self) -> bool {
        // Infinity was considered a valid G1Element in older Relic versions
        // For historical compatibililty this behavior is maintained.
        unsafe { blst_p1_is_inf(&self.0) || blst_p1_in_g1(&self.0) }
    }

    pub fn is_inf(&self) -> bool {
        unsafe { blst_p1_is_inf(&self.0) }
    }

    pub fn negate(&mut self) {
        unsafe {
            blst_p1_cneg(&mut self.0, true);
        }
    }

    pub fn scalar_multiply(&mut self, int_bytes: &[u8]) {
        unsafe {
            let mut scalar = MaybeUninit::<blst_scalar>::uninit();
            blst_scalar_from_be_bytes(scalar.as_mut_ptr(), int_bytes.as_ptr(), int_bytes.len());
            blst_p1_mult(&mut self.0, &self.0, scalar.as_ptr().cast::<u8>(), 256);
        }
    }

    pub fn get_fingerprint(&self) -> u32 {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        let hash: [u8; 32] = hasher.finalize();
        u32::from_be_bytes(hash[0..4].try_into().unwrap())
    }
}

impl PartialEq for PublicKey {
    fn eq(&self, other: &Self) -> bool {
        unsafe { blst_p1_is_equal(&self.0, &other.0) }
    }
}
impl Eq for PublicKey {}

#[cfg(feature = "serde")]
impl serde::Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        chia_serde::ser_bytes(&self.to_bytes(), serializer, true)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::from_bytes(&chia_serde::de_bytes(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Streamable for PublicKey {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.to_bytes());
    }

    fn stream(&self, out: &mut Vec<u8>) -> chia_traits::Result<()> {
        out.extend_from_slice(&self.to_bytes());
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> chia_traits::Result<Self> {
        let input = read_bytes(input, 48)?.try_into().unwrap();
        if TRUSTED {
            Ok(Self::from_bytes_unchecked(input)?)
        } else {
            Ok(Self::from_bytes(input)?)
        }
    }
}

impl Hash for PublicKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.to_bytes());
    }
}

impl Neg for PublicKey {
    type Output = PublicKey;
    fn neg(mut self) -> Self::Output {
        self.negate();
        self
    }
}

impl Neg for &PublicKey {
    type Output = PublicKey;
    fn neg(self) -> Self::Output {
        let mut ret = *self;
        ret.negate();
        ret
    }
}

impl AddAssign<&PublicKey> for PublicKey {
    fn add_assign(&mut self, rhs: &PublicKey) {
        unsafe {
            blst_p1_add_or_double(&mut self.0, &self.0, &rhs.0);
        }
    }
}

impl SubAssign<&PublicKey> for PublicKey {
    fn sub_assign(&mut self, rhs: &PublicKey) {
        unsafe {
            let mut neg = *rhs;
            blst_p1_cneg(&mut neg.0, true);
            blst_p1_add_or_double(&mut self.0, &self.0, &neg.0);
        }
    }
}

impl Add<&PublicKey> for &PublicKey {
    type Output = PublicKey;
    fn add(self, rhs: &PublicKey) -> PublicKey {
        let p1 = unsafe {
            let mut ret = MaybeUninit::<blst_p1>::uninit();
            blst_p1_add_or_double(ret.as_mut_ptr(), &self.0, &rhs.0);
            ret.assume_init()
        };
        PublicKey(p1)
    }
}

impl Add<&PublicKey> for PublicKey {
    type Output = PublicKey;
    fn add(mut self, rhs: &PublicKey) -> PublicKey {
        unsafe {
            blst_p1_add_or_double(&mut self.0, &self.0, &rhs.0);
            self
        }
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!(
            "<G1Element {}>",
            &hex::encode(self.to_bytes())
        ))
    }
}

impl DerivableKey for PublicKey {
    fn derive_unhardened(&self, idx: u32) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        hasher.update(idx.to_be_bytes());
        let digest: [u8; 32] = hasher.finalize();

        let p1 = unsafe {
            let mut nonce = MaybeUninit::<blst_scalar>::uninit();
            blst_scalar_from_lendian(nonce.as_mut_ptr(), digest.as_ptr());
            let mut bte = MaybeUninit::<[u8; 48]>::uninit();
            blst_bendian_from_scalar(bte.as_mut_ptr().cast::<u8>(), nonce.as_ptr());
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_mult(
                p1.as_mut_ptr(),
                blst_p1_generator(),
                bte.as_ptr().cast::<u8>(),
                256,
            );
            blst_p1_add(p1.as_mut_ptr(), p1.as_mut_ptr(), &self.0);
            p1.assume_init()
        };
        PublicKey(p1)
    }
}

pub(crate) const DST: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_AUG_";

pub fn hash_to_g1(msg: &[u8]) -> PublicKey {
    hash_to_g1_with_dst(msg, DST)
}

pub fn hash_to_g1_with_dst(msg: &[u8], dst: &[u8]) -> PublicKey {
    let p1 = unsafe {
        let mut p1 = MaybeUninit::<blst_p1>::uninit();
        blst_hash_to_g1(
            p1.as_mut_ptr(),
            msg.as_ptr(),
            msg.len(),
            dst.as_ptr(),
            dst.len(),
            std::ptr::null(),
            0,
        );
        p1.assume_init()
    };
    PublicKey(p1)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pymethods]
impl PublicKey {
    #[classattr]
    pub const SIZE: usize = 48;

    #[new]
    pub fn init() -> Self {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "generator")]
    pub fn py_generator() -> Self {
        Self::generator()
    }

    pub fn verify(&self, signature: &crate::Signature, msg: &[u8]) -> bool {
        crate::verify(signature, self, msg)
    }

    pub fn pair(&self, other: &crate::Signature) -> crate::GTElement {
        other.pair(self)
    }

    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "PublicKey does not support from_parent().",
        ))
    }

    #[pyo3(name = "get_fingerprint")]
    pub fn py_get_fingerprint(&self) -> u32 {
        self.get_fingerprint()
    }

    #[pyo3(name = "derive_unhardened")]
    #[must_use]
    pub fn py_derive_unhardened(&self, idx: u32) -> Self {
        self.derive_unhardened(idx)
    }

    pub fn __str__(&self) -> String {
        hex::encode(self.to_bytes())
    }

    #[must_use]
    pub fn __add__(&self, rhs: &Self) -> Self {
        self + rhs
    }

    pub fn __iadd__(&mut self, rhs: &Self) {
        *self += rhs;
    }
}

#[cfg(feature = "py-bindings")]
mod pybindings {
    use super::*;

    use crate::parse_hex::parse_hex_string;

    use chia_traits::{FromJsonDict, ToJsonDict};

    impl ToJsonDict for PublicKey {
        fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
            let bytes = self.to_bytes();
            Ok(("0x".to_string() + &hex::encode(bytes))
                .into_pyobject(py)?
                .into_any()
                .unbind())
        }
    }

    impl FromJsonDict for PublicKey {
        fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
            Ok(Self::from_bytes(
                parse_hex_string(o, 48, "PublicKey")?
                    .as_slice()
                    .try_into()
                    .unwrap(),
            )?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretKey;
    use hex::FromHex;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rstest::rstest;

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

    #[test]
    fn test_from_bytes() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 48];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            // clear the bits that mean infinity
            data[0] = 0x80;
            // just any random bytes are not a valid key and should fail
            match PublicKey::from_bytes(&data) {
                Err(Error::InvalidPublicKey(err)) => {
                    assert!([
                        BLST_ERROR::BLST_BAD_ENCODING,
                        BLST_ERROR::BLST_POINT_NOT_ON_CURVE
                    ]
                    .contains(&err));
                }
                Err(e) => {
                    panic!("unexpected error from_bytes(): {e}");
                }
                Ok(v) => {
                    panic!("unexpected value from_bytes(): {v:?}");
                }
            }
        }
    }

    #[rstest]
    #[case("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001", Error::G1NotCanonical)]
    #[case("c08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", Error::G1NotCanonical)]
    #[case("c80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", Error::G1NotCanonical)]
    #[case("e00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", Error::G1NotCanonical)]
    #[case("d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", Error::G1NotCanonical)]
    #[case("800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", Error::G1InfinityNotZero)]
    #[case("400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", Error::G1InfinityInvalidBits)]
    fn test_from_bytes_failures(#[case] input: &str, #[case()] error: Error) {
        let bytes: [u8; 48] = hex::decode(input).unwrap().try_into().unwrap();
        assert_eq!(PublicKey::from_bytes(&bytes).unwrap_err(), error);
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
        assert_eq!(pk.get_fingerprint(), 651_010_559);
    }

    #[test]
    fn test_aggregate_pubkey() {
        // from blspy import PrivateKey
        // from blspy import AugSchemeMPL
        // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
        // pk = sk.get_g1()
        // pk + pk
        // <G1Element b1b8033286299e7f238aede0d3fea48d133a1e233139085f72c102c2e6cc1f8a4ea64ed2838c10bbd2ef8f78ef271bf3>
        // pk + pk + pk
        // <G1Element a8bc2047d90c04a12e8c38050ec0feb4417b4d5689165cd2cea8a7903aad1778e36548a46d427b5ec571364515e456d6>

        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        let pk = sk.public_key();
        let pk2 = &pk + &pk;
        let pk3 = &pk + &pk + &pk;

        assert_eq!(pk2, PublicKey::from_bytes(&<[u8; 48]>::from_hex("b1b8033286299e7f238aede0d3fea48d133a1e233139085f72c102c2e6cc1f8a4ea64ed2838c10bbd2ef8f78ef271bf3").unwrap()).unwrap());
        assert_eq!(pk3, PublicKey::from_bytes(&<[u8; 48]>::from_hex("a8bc2047d90c04a12e8c38050ec0feb4417b4d5689165cd2cea8a7903aad1778e36548a46d427b5ec571364515e456d6").unwrap()).unwrap());
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

    #[test]
    fn test_default_is_inf() {
        let pk = PublicKey::default();
        assert!(pk.is_inf());
    }

    #[test]
    fn test_infinity() {
        let mut data = [0u8; 48];
        data[0] = 0xc0;
        let pk = PublicKey::from_bytes(&data).unwrap();
        assert!(pk.is_inf());
    }

    #[test]
    fn test_is_inf() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..500 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let pk = sk.public_key();
            assert!(!pk.is_inf());
        }
    }

    #[test]
    fn test_hash() {
        fn hash<T: Hash>(v: T) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            let mut h = DefaultHasher::new();
            v.hash(&mut h);
            h.finish()
        }

        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        rng.fill(data.as_mut_slice());

        let sk = SecretKey::from_seed(&data);
        let pk1 = sk.public_key();
        let pk2 = pk1.derive_unhardened(1);
        let pk3 = pk1.derive_unhardened(2);

        assert!(hash(pk2) != hash(pk3));
        assert!(hash(pk1.derive_unhardened(42)) == hash(pk1.derive_unhardened(42)));
    }

    #[test]
    fn test_debug() {
        let mut data = [0u8; 48];
        data[0] = 0xc0;
        let pk = PublicKey::from_bytes(&data).unwrap();
        assert_eq!(
            format!("{pk:?}"),
            format!("<G1Element {}>", hex::encode(data))
        );
    }

    #[test]
    fn test_generator() {
        assert_eq!(
            hex::encode(PublicKey::generator().to_bytes()),
            "97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb"
        );
    }

    #[test]
    fn test_from_integer() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            // this integer may not exceed the group order, so leave the top
            // byte as 0
            rng.fill(&mut data[1..]);

            let g1 = PublicKey::from_integer(&data);
            let expected_g1 = SecretKey::from_bytes(&data)
                .expect("invalid public key")
                .public_key();
            assert_eq!(g1, expected_g1);
        }
    }

    // test cases from zksnark test in chia_rs
    #[rstest]
    #[case("06f6ba2972ab1c83718d747b2d55cca96d08729b1ea5a3ab3479b8efe2d455885abf65f58d1507d7f260cd2a4687db821171c9d8dc5c0f5c3c4fd64b26cf93ff28b2e683c409fb374c4e26cc548c6f7cef891e60b55e6115bb38bbe97822e4d4", "a6f6ba2972ab1c83718d747b2d55cca96d08729b1ea5a3ab3479b8efe2d455885abf65f58d1507d7f260cd2a4687db82")]
    #[case("127271e81a1cb5c08a68694fcd5bd52f475d545edd4fbd49b9f6ec402ee1973f9f4102bf3bfccdcbf1b2f862af89a1340d40795c1c09d1e10b1acfa0f3a97a71bf29c11665743fa8d30e57e450b8762959571d6f6d253b236931b93cf634e7cf", "b27271e81a1cb5c08a68694fcd5bd52f475d545edd4fbd49b9f6ec402ee1973f9f4102bf3bfccdcbf1b2f862af89a134")]
    #[case("0fe94ac2d68d39d9207ea0cae4bb2177f7352bd754173ed27bd13b4c156f77f8885458886ee9fbd212719f27a96397c110fa7b4f898b1c45c2e82c5d46b52bdad95cae8299d4fd4556ae02baf20a5ec989fc62f28c8b6b3df6dc696f2afb6e20", "afe94ac2d68d39d9207ea0cae4bb2177f7352bd754173ed27bd13b4c156f77f8885458886ee9fbd212719f27a96397c1")]
    #[case("13aedc305adfdbc854aa105c41085618484858e6baa276b176fd89415021f7a0c75ff4f9ec39f482f142f1b54c11144815e519df6f71b1db46c83b1d2bdf381fc974059f3ccd87ed5259221dc37c50c3be407b58990d14b6d5bb79dad9ab8c42", "b3aedc305adfdbc854aa105c41085618484858e6baa276b176fd89415021f7a0c75ff4f9ec39f482f142f1b54c111448")]
    fn test_from_uncompressed(#[case] input: &str, #[case] expect: &str) {
        let input = hex::decode(input).unwrap();
        let g1 = PublicKey::from_uncompressed(input.as_slice().try_into().unwrap()).unwrap();
        let compressed = g1.to_bytes();
        assert_eq!(hex::encode(compressed), expect);
    }

    #[test]
    fn test_negate_roundtrip() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            // this integer may not exceed the group order, so leave the top
            // byte as 0
            rng.fill(&mut data[1..]);

            let g1 = PublicKey::from_integer(&data);
            let mut g1_neg = g1;
            g1_neg.negate();
            assert!(g1_neg != g1);

            g1_neg.negate();
            assert!(g1_neg == g1);
        }
    }

    #[test]
    fn test_negate_infinity() {
        let g1 = PublicKey::default();
        let mut g1_neg = g1;
        // negate on infinity is a no-op
        g1_neg.negate();
        assert!(g1_neg == g1);
    }

    #[test]
    fn test_negate() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            // this integer may not exceed the group order, so leave the top
            // byte as 0
            rng.fill(&mut data[1..]);

            let g1 = PublicKey::from_integer(&data);
            let mut g1_neg = g1;
            g1_neg.negate();

            let mut g1_double = g1;
            // adding the negative undoes adding the positive
            g1_double += &g1;
            assert!(g1_double != g1);
            g1_double += &g1_neg;
            assert!(g1_double == g1);
        }
    }

    #[test]
    fn test_scalar_multiply() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            // this integer may not exceed the group order, so leave the top
            // byte as 0
            rng.fill(&mut data[1..]);

            let mut g1 = PublicKey::from_integer(&data);
            let mut g1_double = g1;
            g1_double += &g1;
            assert!(g1_double != g1);
            // scalar multiply by 2 is the same as adding oneself
            g1.scalar_multiply(&[2]);
            assert!(g1_double == g1);
        }
    }

    #[test]
    fn test_hash_to_g1_different_dst() {
        const DEFAULT_DST: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_AUG_";
        const CUSTOM_DST: &[u8] = b"foobar";

        let mut rng = StdRng::seed_from_u64(1337);
        let mut msg = [0u8; 32];
        for _i in 0..50 {
            rng.fill(&mut msg);
            let default_hash = hash_to_g1(&msg);
            assert_eq!(default_hash, hash_to_g1_with_dst(&msg, DEFAULT_DST));
            assert!(default_hash != hash_to_g1_with_dst(&msg, CUSTOM_DST));
        }
    }

    // test cases from clvm_rs
    #[rstest]
    #[case("abcdef0123456789", "88e7302bf1fa8fcdecfb96f6b81475c3564d3bcaf552ccb338b1c48b9ba18ab7195c5067fe94fb216478188c0a3bef4a")]
    fn test_hash_to_g1(#[case] input: &str, #[case] expect: &str) {
        let g1 = hash_to_g1(input.as_bytes());
        assert_eq!(hex::encode(g1.to_bytes()), expect);
    }

    // test cases from clvm_rs
    #[rstest]
    #[case("abcdef0123456789", "BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_", "8dd8e3a9197ddefdc25dde980d219004d6aa130d1af9b1808f8b2b004ae94484ac62a08a739ec7843388019a79c437b0")]
    #[case("abcdef0123456789", "BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_AUG_", "88e7302bf1fa8fcdecfb96f6b81475c3564d3bcaf552ccb338b1c48b9ba18ab7195c5067fe94fb216478188c0a3bef4a")]
    fn test_hash_to_g1_with_dst(#[case] input: &str, #[case] dst: &str, #[case] expect: &str) {
        let g1 = hash_to_g1_with_dst(input.as_bytes(), dst.as_bytes());
        assert_eq!(hex::encode(g1.to_bytes()), expect);
    }
}

#[cfg(test)]
#[cfg(feature = "py-bindings")]
mod pytests {
    use super::*;
    use crate::SecretKey;
    use pyo3::Python;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rstest::rstest;

    #[test]
    fn test_json_dict_roundtrip() {
        pyo3::prepare_freethreaded_python();
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let pk = sk.public_key();
            Python::with_gil(|py| {
                let string = pk.to_json_dict(py).expect("to_json_dict");
                let py_class = py.get_type::<PublicKey>();
                let pk2: PublicKey = PublicKey::from_json_dict(&py_class, py, string.bind(py))
                    .unwrap()
                    .extract(py)
                    .unwrap();
                assert_eq!(pk, pk2);
            });
        }
    }

    #[rstest]
    #[case("0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e", "PublicKey, invalid length 47 expected 48")]
    #[case("0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f00", "PublicKey, invalid length 49 expected 48")]
    #[case("000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e", "PublicKey, invalid length 47 expected 48")]
    #[case("000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f00", "PublicKey, invalid length 49 expected 48")]
    #[case("0x00r102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f", "invalid hex")]
    fn test_json_dict(#[case] input: &str, #[case] msg: &str) {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_class = py.get_type::<PublicKey>();
            let err = PublicKey::from_json_dict(
                &py_class,
                py,
                &input.to_string().into_pyobject(py).unwrap().into_any(),
            )
            .unwrap_err();
            assert_eq!(err.value(py).to_string(), msg.to_string());
        });
    }
}
