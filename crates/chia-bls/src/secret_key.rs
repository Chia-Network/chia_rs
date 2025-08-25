use crate::{DerivableKey, Error, PublicKey, Result};
use blst::*;
use chia_sha2::Sha256;
use chia_traits::{read_bytes, Streamable};
use hkdf::HkdfExtract;
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
use std::ops::{Add, AddAssign};

#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(frozen, name = "PrivateKey"),
    derive(chia_py_streamable_macro::PyStreamable)
)]
#[derive(PartialEq, Eq, Clone)]
pub struct SecretKey(pub(crate) blst_scalar);

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for SecretKey {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut seed = [0_u8; 32];
        let _ = u.fill_buffer(seed.as_mut_slice());
        Ok(Self::from_seed(&seed))
    }
}

fn flip_bits(input: [u8; 32]) -> [u8; 32] {
    let mut ret = [0; 32];
    for i in 0..32 {
        ret[i] = input[i] ^ 0xff;
    }
    ret
}

fn ikm_to_lamport_sk(ikm: &[u8; 32], salt: [u8; 4]) -> [u8; 255 * 32] {
    let mut extracter = HkdfExtract::<sha2::Sha256>::new(Some(&salt));
    extracter.input_ikm(ikm);
    let (_, h) = extracter.finalize();

    let mut output = [0_u8; 255 * 32];
    h.expand(&[], &mut output).unwrap();
    output
}

fn to_lamport_pk(ikm: [u8; 32], idx: u32) -> [u8; 32] {
    let not_ikm = flip_bits(ikm);
    let salt = idx.to_be_bytes();

    let mut lamport0 = ikm_to_lamport_sk(&ikm, salt);
    let mut lamport1 = ikm_to_lamport_sk(&not_ikm, salt);

    for i in (0..32 * 255).step_by(32) {
        let hash = sha256(&lamport0[i..i + 32]);
        lamport0[i..i + 32].copy_from_slice(&hash);
    }
    for i in (0..32 * 255).step_by(32) {
        let hash = sha256(&lamport1[i..i + 32]);
        lamport1[i..i + 32].copy_from_slice(&hash);
    }

    let mut hasher = Sha256::new();
    hasher.update(lamport0);
    hasher.update(lamport1);
    hasher.finalize()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize()
}

pub fn is_all_zero(buf: &[u8]) -> bool {
    let (prefix, aligned, suffix) = unsafe { buf.align_to::<u128>() };

    prefix.iter().all(|&x| x == 0)
        && suffix.iter().all(|&x| x == 0)
        && aligned.iter().all(|&x| x == 0)
}

impl SecretKey {
    /// # Panics
    ///
    /// Panics if the seed produces an invalid SecretKey.
    #[must_use]
    pub fn from_seed(seed: &[u8]) -> Self {
        // described here:
        // https://eips.ethereum.org/EIPS/eip-2333#derive_master_sk
        assert!(seed.len() >= 32);

        let bytes = unsafe {
            let mut scalar = MaybeUninit::<blst_scalar>::uninit();
            blst_keygen_v3(
                scalar.as_mut_ptr(),
                seed.as_ptr(),
                seed.len(),
                std::ptr::null(),
                0,
            );
            let mut bytes = MaybeUninit::<[u8; 32]>::uninit();
            blst_bendian_from_scalar(bytes.as_mut_ptr().cast::<u8>(), &scalar.assume_init());
            bytes.assume_init()
        };
        Self::from_bytes(&bytes).expect("from_seed")
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self> {
        let pk = unsafe {
            let mut pk = MaybeUninit::<blst_scalar>::uninit();
            blst_scalar_from_bendian(pk.as_mut_ptr(), bytes.as_ptr());
            pk.assume_init()
        };

        if is_all_zero(bytes) {
            // don't check anything else, we allow zero private key
            return Ok(Self(pk));
        }

        if unsafe { !blst_sk_check(&pk) } {
            return Err(Error::SecretKeyGroupOrder);
        }

        Ok(Self(pk))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; 32]>::uninit();
            blst_bendian_from_scalar(bytes.as_mut_ptr().cast::<u8>(), &self.0);
            bytes.assume_init()
        }
    }

    pub fn public_key(&self) -> PublicKey {
        let p1 = unsafe {
            let mut p1 = MaybeUninit::<blst_p1>::uninit();
            blst_sk_to_pk_in_g1(p1.as_mut_ptr(), &self.0);
            p1.assume_init()
        };
        PublicKey(p1)
    }

    #[must_use]
    pub fn derive_hardened(&self, idx: u32) -> SecretKey {
        // described here:
        // https://eips.ethereum.org/EIPS/eip-2333#derive_child_sk
        SecretKey::from_seed(to_lamport_pk(self.to_bytes(), idx).as_slice())
    }
}

impl Streamable for SecretKey {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(self.to_bytes());
    }

    fn stream(&self, out: &mut Vec<u8>) -> chia_traits::chia_error::Result<()> {
        out.extend_from_slice(&self.to_bytes());
        Ok(())
    }

    fn parse<const TRUSTED: bool>(
        input: &mut Cursor<&[u8]>,
    ) -> chia_traits::chia_error::Result<Self> {
        Ok(Self::from_bytes(
            read_bytes(input, 32)?.try_into().unwrap(),
        )?)
    }
}

impl Hash for SecretKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.to_bytes());
    }
}

impl Add<&SecretKey> for &SecretKey {
    type Output = SecretKey;
    fn add(self, rhs: &SecretKey) -> SecretKey {
        let scalar = unsafe {
            let mut ret = MaybeUninit::<blst_scalar>::uninit();
            blst_sk_add_n_check(ret.as_mut_ptr(), &self.0, &rhs.0);
            ret.assume_init()
        };
        SecretKey(scalar)
    }
}

impl Add<&SecretKey> for SecretKey {
    type Output = SecretKey;
    fn add(mut self, rhs: &SecretKey) -> SecretKey {
        unsafe {
            blst_sk_add_n_check(&mut self.0, &self.0, &rhs.0);
            self
        }
    }
}

impl AddAssign<&SecretKey> for SecretKey {
    fn add_assign(&mut self, rhs: &SecretKey) {
        unsafe {
            blst_sk_add_n_check(&mut self.0, &self.0, &rhs.0);
        }
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!(
            "<PrivateKey {}>",
            &hex::encode(self.to_bytes())
        ))
    }
}

impl DerivableKey for SecretKey {
    fn derive_unhardened(&self, idx: u32) -> Self {
        let pk = self.public_key();

        let mut hasher = Sha256::new();
        hasher.update(pk.to_bytes());
        hasher.update(idx.to_be_bytes());
        let digest = hasher.finalize();

        let scalar = unsafe {
            let mut scalar = MaybeUninit::<blst_scalar>::uninit();
            let success =
                blst_scalar_from_be_bytes(scalar.as_mut_ptr(), digest.as_ptr(), digest.len());
            assert!(success);
            let success = blst_sk_add_n_check(scalar.as_mut_ptr(), scalar.as_ptr(), &self.0);
            assert!(success);
            scalar.assume_init()
        };
        Self(scalar)
    }
}

#[cfg(feature = "py-bindings")]
#[pyo3::pymethods]
impl SecretKey {
    #[classattr]
    pub const PRIVATE_KEY_SIZE: usize = 32;

    #[pyo3(signature = (msg, final_pk=None))]
    pub fn sign(&self, msg: &[u8], final_pk: Option<PublicKey>) -> crate::Signature {
        match final_pk {
            Some(prefix) => {
                let mut aug_msg = prefix.to_bytes().to_vec();
                aug_msg.extend_from_slice(msg);
                crate::sign_raw(self, aug_msg)
            }
            None => crate::sign(self, msg),
        }
    }

    pub fn get_g1(&self) -> PublicKey {
        self.public_key()
    }

    #[pyo3(name = "public_key")]
    pub fn py_public_key(&self) -> PublicKey {
        self.public_key()
    }

    pub fn __str__(&self) -> String {
        hex::encode(self.to_bytes())
    }

    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "SecretKey does not support from_parent().",
        ))
    }

    #[pyo3(name = "derive_hardened")]
    #[must_use]
    pub fn py_derive_hardened(&self, idx: u32) -> Self {
        self.derive_hardened(idx)
    }

    #[pyo3(name = "derive_unhardened")]
    #[must_use]
    pub fn py_derive_unhardened(&self, idx: u32) -> Self {
        self.derive_unhardened(idx)
    }

    #[pyo3(name = "from_seed")]
    #[staticmethod]
    pub fn py_from_seed(seed: &[u8]) -> Self {
        Self::from_seed(seed)
    }
}

#[cfg(feature = "py-bindings")]
mod pybindings {
    use super::*;

    use crate::parse_hex::parse_hex_string;

    use chia_traits::{FromJsonDict, ToJsonDict};

    impl ToJsonDict for SecretKey {
        fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
            let bytes = self.to_bytes();
            Ok(("0x".to_string() + &hex::encode(bytes))
                .into_pyobject(py)?
                .into_any()
                .unbind())
        }
    }

    impl FromJsonDict for SecretKey {
        fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
            Ok(Self::from_bytes(
                parse_hex_string(o, 32, "PrivateKey")?
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
    use hex::FromHex;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    #[test]
    fn test_make_key() {
        // test vectors from:
        // from chia.util.keychain import KeyDataSecrets
        // print(KeyDataSecrets.from_mnemonic(phrase)["privatekey"])

        // (seed, secret-key)
        let test_cases = &[
        ("fc795be0c3f18c50dddb34e72179dc597d64055497ecc1e69e2e56a5409651bc139aae8070d4df0ea14d8d2a518a9a00bb1cc6e92e053fe34051f6821df9164c",
            "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"),
        ("b873212f885ccffbf4692afcb84bc2e55886de2dfa07d90f5c3c239abc31c0a6ce047e30fd8bf6a281e71389aa82d73df74c7bbfb3b06b4639a5cee775cccd3c",
            "35d65c35d926f62ba2dd128754ddb556edb4e2c926237ab9e02a23e7b3533613"),
        ("3e066d7dee2dbf8fcd3fe240a3975658ca118a8f6f4ca81cf99104944604b05a5090a79d99e545704b914ca0397fedb82fd00fd6a72098703709c891a065ee49",
            "59095c391107936599b7ee6f09067979b321932bd62e23c7f53ed5fb19f851f6")
    ];

        for (seed, sk) in test_cases {
            assert_eq!(
                SecretKey::from_seed(&<[u8; 64]>::from_hex(seed).unwrap())
                    .to_bytes()
                    .to_vec(),
                Vec::<u8>::from_hex(sk).unwrap()
            );
        }
    }

    #[test]
    fn test_derive_unhardened() {
        // test vectors from:
        // from blspy import AugSchemeMPL
        // from blspy import PrivateKey
        // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
        // AugSchemeMPL.derive_child_sk_unhardened(sk, 0)
        // AugSchemeMPL.derive_child_sk_unhardened(sk, 1)
        // AugSchemeMPL.derive_child_sk_unhardened(sk, 2)
        // AugSchemeMPL.derive_child_sk_unhardened(sk, 3)
        // <PrivateKey 399638f99d446500f3c3a363f24c2b0634ad7caf646f503455093f35f29290bd>
        // <PrivateKey 3dcb4098ad925d8940e2f516d2d5a4dbab393db928a8c6cb06b93066a09a843a>
        // <PrivateKey 13115c8fb68a3d667938dac2ffc6b867a4a0f216bbb228aa43d6bdde14245575>
        // <PrivateKey 52e7e9f2fb51f2c5705aea8e11ac82737b95e664ae578f015af22031d956f92b>

        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let derived_hex = [
            "399638f99d446500f3c3a363f24c2b0634ad7caf646f503455093f35f29290bd",
            "3dcb4098ad925d8940e2f516d2d5a4dbab393db928a8c6cb06b93066a09a843a",
            "13115c8fb68a3d667938dac2ffc6b867a4a0f216bbb228aa43d6bdde14245575",
            "52e7e9f2fb51f2c5705aea8e11ac82737b95e664ae578f015af22031d956f92b",
        ];
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        for (i, hex) in derived_hex.iter().enumerate() {
            let derived = sk.derive_unhardened(i as u32);
            assert_eq!(derived.to_bytes(), <[u8; 32]>::from_hex(hex).unwrap());
        }
    }

    #[test]
    fn test_public_key() {
        // test vectors from:
        // from blspy import PrivateKey
        // from blspy import AugSchemeMPL
        // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
        // for i in [100, 52312, 352350, 316]:
        //         sk0 = AugSchemeMPL.derive_child_sk_unhardened(sk, i)
        //         print(bytes(sk0).hex())
        //         print(bytes(sk0.get_g1()).hex())

        // secret key, public key
        let test_cases = [
        ("5aac8405befe4cb3748a67177c56df26355f1f98d979afdb0b2f97858d2f71c3",
        "b9de000821a610ef644d160c810e35113742ff498002c2deccd8f1a349e423047e9b3fc17ebfc733dbee8fd902ba2961"),
        ("23f1fb291d3bd7434282578b842d5ea4785994bb89bd2c94896d1b4be6c70ba2",
        "96f304a5885e67abdeab5e1ed0576780a1368777ea7760124834529e8694a1837a20ffea107b9769c4f92a1f6c167e69"),
        ("2bc1d6d6efe58d365c29ccb7ad12c8457c0eec70a29003073692ac4cb1cd7ba2",
        "b10568446def64b17fc9b6d614ae036deaac3f2d654e12e45ea04b19208246e0d760e8826426e97f9f0666b7ce340d75"),
        ("2bfc8672d859700e30aa6c8edc24a8ce9e6dc53bb1ef936f82de722847d05b9e",
        "9641472acbd6af7e5313d2500791b87117612af43eef929cf7975aaaa5a203a32698a8ef53763a84d90ad3f00b86ad66"),
        ("3311f883dad1e39c52bf82d5870d05371c0b1200576287b5160808f55568151b",
        "928ea102b5a3e3efe4f4c240d3458a568dfeb505e02901a85ed70a384944b0c08c703a35245322709921b8f2b7f5e54a"),
        ];

        for (sk_hex, pk_hex) in test_cases {
            let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
            let pk = sk.public_key();
            assert_eq!(
                pk,
                PublicKey::from_bytes(&<[u8; 48]>::from_hex(pk_hex).unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn test_derive_hardened() {
        // test vectors from:
        // from blspy import AugSchemeMPL
        // from blspy import PrivateKey
        // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
        // AugSchemeMPL.derive_child_sk(sk, 0)
        // AugSchemeMPL.derive_child_sk(sk, 1)
        // AugSchemeMPL.derive_child_sk(sk, 2)
        // AugSchemeMPL.derive_child_sk(sk, 3)
        // <PrivateKey 05eccb2d70e814f51a30d8b9965505605c677afa97228fa2419db583a8121db9>
        // <PrivateKey 612ae96bdce2e9bc01693ac579918fbb559e04ec365cce9b66bb80e328f62c46>
        // <PrivateKey 5df14a0a34fd6c30a80136d4103f0a93422ce82d5c537bebbecbc56e19fee5b9>
        // <PrivateKey 3ea55db88d9a6bf5f1d9c9de072e3c9a56b13f4156d72fca7880cd39b4bd4fdc>

        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let derived_hex = [
            "05eccb2d70e814f51a30d8b9965505605c677afa97228fa2419db583a8121db9",
            "612ae96bdce2e9bc01693ac579918fbb559e04ec365cce9b66bb80e328f62c46",
            "5df14a0a34fd6c30a80136d4103f0a93422ce82d5c537bebbecbc56e19fee5b9",
            "3ea55db88d9a6bf5f1d9c9de072e3c9a56b13f4156d72fca7880cd39b4bd4fdc",
        ];
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        for (i, hex) in derived_hex.iter().enumerate() {
            let derived = sk.derive_hardened(i as u32);
            assert_eq!(derived.to_bytes(), <[u8; 32]>::from_hex(hex).unwrap());
        }
    }

    #[test]
    fn test_debug() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        assert_eq!(format!("{sk:?}"), format!("<PrivateKey {sk_hex}>"));
    }

    #[test]
    fn test_hash() {
        fn hash<T: Hash>(v: &T) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            let mut h = DefaultHasher::new();
            v.hash(&mut h);
            h.finish()
        }

        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        rng.fill(data.as_mut_slice());

        let sk1 = SecretKey::from_seed(&data);
        let sk2 = SecretKey::from_seed(&data);

        rng.fill(data.as_mut_slice());
        let sk3 = SecretKey::from_seed(&data);

        assert!(hash(&sk1) == hash(&sk2));
        assert!(hash(&sk1) != hash(&sk3));
    }

    #[test]
    fn test_from_bytes() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            // make the bytes exceed q
            data[0] |= 0x80;
            // just any random bytes are not a valid key and should fail
            assert_eq!(
                SecretKey::from_bytes(&data).unwrap_err(),
                Error::SecretKeyGroupOrder
            );
        }
    }

    #[test]
    fn test_from_bytes_zero() {
        let data = [0u8; 32];
        let _sk = SecretKey::from_bytes(&data).unwrap();
    }

    #[test]
    fn test_aggregate_secret_key() {
        let sk_hex = "5aac8405befe4cb3748a67177c56df26355f1f98d979afdb0b2f97858d2f71c3";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        let sk2 = &sk + &sk;
        let sk3 = &sk + &sk + &sk;

        assert_eq!(
            sk2,
            SecretKey::from_bytes(
                &<[u8; 32]>::from_hex(
                    "416b60b8545f1c1eb5daf626ef0be64717009b2eb2f503b7165f2f0c1a5ee385"
                )
                .unwrap()
            )
            .unwrap()
        );

        assert_eq!(
            sk3,
            SecretKey::from_bytes(
                &<[u8; 32]>::from_hex(
                    "282a3d6ae9bfeb89f72b853661c0ed67f8a216c48c705793218ec692a78e5547"
                )
                .unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn test_roundtrip() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let bytes = sk.to_bytes();
            let sk2 = SecretKey::from_bytes(&bytes).unwrap();
            assert_eq!(sk, sk2);
            assert_eq!(sk.public_key(), sk2.public_key());
        }
    }
}

#[cfg(test)]
#[cfg(feature = "py-bindings")]
mod pytests {
    use super::*;
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
            Python::with_gil(|py| {
                let string = sk.to_json_dict(py).expect("to_json_dict");
                let py_class = py.get_type::<SecretKey>();
                let sk2 = SecretKey::from_json_dict(&py_class, py, string.bind(py))
                    .unwrap()
                    .extract(py)
                    .unwrap();
                assert_eq!(sk, sk2);
                assert_eq!(sk.public_key(), sk2.public_key());
            });
        }
    }

    #[rstest]
    #[case(
        "0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e",
        "PrivateKey, invalid length 31 expected 32"
    )]
    #[case(
        "0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f00",
        "PrivateKey, invalid length 33 expected 32"
    )]
    #[case(
        "000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f00",
        "PrivateKey, invalid length 33 expected 32"
    )]
    #[case(
        "000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e",
        "PrivateKey, invalid length 31 expected 32"
    )]
    #[case(
        "0r0102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f",
        "invalid hex"
    )]
    fn test_json_dict(#[case] input: &str, #[case] msg: &str) {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_class = py.get_type::<SecretKey>();
            let err = SecretKey::from_json_dict(
                &py_class,
                py,
                &input.to_string().into_pyobject(py).unwrap().into_any(),
            )
            .unwrap_err();
            assert_eq!(err.value(py).to_string(), msg.to_string());
        });
    }
}
