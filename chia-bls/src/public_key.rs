use crate::secret_key::is_all_zero;
use crate::{DerivableKey, Error, Result};
use blst::*;
use chia_traits::{read_bytes, Streamable};
use clvm_traits::{FromClvm, ToClvm};
use clvmr::allocator::{Allocator, NodePtr, SExp};
use sha2::{digest::FixedOutput, Digest, Sha256};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign};

#[cfg(feature = "py-bindings")]
use crate::{GTElement, Signature};
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use chia_traits::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use chia_traits::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods, IntoPy, PyAny, PyObject, PyResult, Python};

#[cfg_attr(
    feature = "py-bindings",
    pyclass(name = "G1Element"),
    derive(PyStreamable)
)]
#[derive(Clone, Default)]
pub struct PublicKey(pub(crate) blst_p1);

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
        } else {
            if (bytes[0] & 0xc0) != 0x80 {
                return Err(Error::G1InfinityInvalidBits);
            }
            if zeros_only {
                return Err(Error::G1InfinityNotZero);
            }
        }

        let p1 = unsafe {
            let mut p1_affine = MaybeUninit::<blst_p1_affine>::uninit();
            let ret = blst_p1_uncompress(p1_affine.as_mut_ptr(), bytes as *const u8);
            if ret != BLST_ERROR::BLST_SUCCESS {
                return Err(Error::InvalidPublicKey(ret));
            }
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_p1_from_affine(p1.as_mut_ptr(), &p1_affine.assume_init());
            p1.assume_init()
        };
        Ok(Self(p1))
    }

    pub fn from_bytes(bytes: &[u8; 48]) -> Result<Self> {
        let ret = Self::from_bytes_unchecked(bytes)?;
        if !ret.is_valid() {
            Err(Error::InvalidPublicKey(BLST_ERROR::BLST_POINT_NOT_ON_CURVE))
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

#[cfg(feature = "py-bindings")]
#[cfg_attr(feature = "py-bindings", pymethods)]
impl PublicKey {
    #[classattr]
    const SIZE: usize = 48;

    #[new]
    pub fn init() -> Self {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "from_bytes_unchecked")]
    fn py_from_bytes_unchecked(bytes: [u8; Self::SIZE]) -> Result<Self> {
        Self::from_bytes_unchecked(&bytes)
    }

    #[staticmethod]
    pub fn generator() -> Self {
        unsafe { Self(*blst_p1_generator()) }
    }

    pub fn pair(&self, other: &Signature) -> GTElement {
        other.pair(self)
    }

    #[pyo3(name = "get_fingerprint")]
    pub fn py_get_fingerprint(&self) -> u32 {
        self.get_fingerprint()
    }

    pub fn __repr__(&self) -> String {
        let bytes = self.to_bytes();
        format!("<G1Element {}>", &hex::encode(bytes))
    }

    pub fn __add__(&self, rhs: &Self) -> Self {
        self + rhs
    }

    pub fn __iadd__(&mut self, rhs: &Self) {
        *self += rhs;
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

    fn stream(&self, out: &mut Vec<u8>) -> chia_traits::Result<()> {
        out.extend_from_slice(&self.to_bytes());
        Ok(())
    }

    fn parse(input: &mut Cursor<&[u8]>) -> chia_traits::Result<Self> {
        Ok(Self::from_bytes(
            read_bytes(input, 48)?.try_into().unwrap(),
        )?)
    }
}

impl Hash for PublicKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.to_bytes())
    }
}

impl AddAssign<&PublicKey> for PublicKey {
    fn add_assign(&mut self, rhs: &PublicKey) {
        unsafe {
            blst_p1_add_or_double(&mut self.0, &self.0, &rhs.0);
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
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self.to_bytes()))
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for PublicKey {
    fn to_json_dict(&self, py: Python) -> pyo3::PyResult<PyObject> {
        let bytes = self.to_bytes();
        Ok(("0x".to_string() + &hex::encode(bytes)).into_py(py))
    }
}

#[cfg(feature = "py-bindings")]
pub fn parse_hex_string(o: &PyAny, len: usize, name: &str) -> PyResult<Vec<u8>> {
    use pyo3::exceptions::PyValueError;
    let s: String = o.extract()?;
    let s = if let Some(st) = s.strip_prefix("0x") {
        st
    } else {
        &s[..]
    };
    let buf = match hex::decode(s) {
        Err(_) => {
            return Err(PyValueError::new_err("invalid hex"));
        }
        Ok(v) => v,
    };
    if buf.len() != len {
        Err(PyValueError::new_err(format!(
            "{}, invalid length {} expected {}",
            name,
            buf.len(),
            len
        )))
    } else {
        Ok(buf)
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for PublicKey {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        Ok(Self::from_bytes(
            parse_hex_string(o, 48, "PublicKey")?
                .as_slice()
                .try_into()
                .unwrap(),
        )?)
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

impl FromClvm for PublicKey {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> clvm_traits::Result<Self> {
        let blob = match a.sexp(ptr) {
            SExp::Atom => a.atom(ptr),
            _ => {
                return Err(clvm_traits::Error::ExpectedAtom(ptr));
            }
        };
        Self::from_bytes(
            blob.try_into()
                .map_err(|_error| clvm_traits::Error::Custom("invalid size".to_string()))?,
        )
        .map_err(|error| clvm_traits::Error::Custom(error.to_string()))
    }
}

impl ToClvm for PublicKey {
    fn to_clvm(&self, a: &mut Allocator) -> clvm_traits::Result<NodePtr> {
        Ok(a.new_atom(&self.to_bytes())?)
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
        assert_eq!(pk.get_fingerprint(), 651010559);
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
    fn test_hash() {
        fn hash<T: std::hash::Hash>(v: T) -> u64 {
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
        assert_eq!(format!("{:?}", pk), hex::encode(data));
    }

    #[test]
    fn test_to_from_clvm() {
        let mut a = Allocator::new();
        let bytes = hex::decode("997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2").expect("hex::decode()");
        let ptr = a.new_atom(&bytes).expect("new_atom");

        let pk = PublicKey::from_clvm(&a, ptr).expect("from_clvm");
        assert_eq!(pk.to_bytes(), &bytes[..]);

        let pk_ptr = pk.to_clvm(&mut a).expect("to_clvm");
        assert!(a.atom_eq(pk_ptr, ptr));
    }

    #[test]
    fn test_from_clvm_failure() {
        let mut a = Allocator::new();
        let ptr = a.new_pair(a.one(), a.one()).expect("new_pair");
        assert_eq!(
            PublicKey::from_clvm(&a, ptr).unwrap_err(),
            clvm_traits::Error::ExpectedAtom(ptr)
        );
    }
}

#[cfg(test)]
#[cfg(feature = "py-bindings")]
mod pytests {
    use super::*;
    use crate::SecretKey;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rstest::rstest;

    #[test]
    fn test_generator() {
        assert_eq!(
            hex::encode(&PublicKey::generator().to_bytes()),
            "97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb"
        );
    }

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
                let pk2 = PublicKey::from_json_dict(string.as_ref(py)).unwrap();
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
            let err =
                PublicKey::from_json_dict(input.to_string().into_py(py).as_ref(py)).unwrap_err();
            assert_eq!(err.value(py).to_string(), msg.to_string());
        });
    }
}
