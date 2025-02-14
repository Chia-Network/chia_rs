use crate::{Error, GTElement, PublicKey, Result, SecretKey};
use blst::*;
use chia_sha2::Sha256;
use chia_traits::{read_bytes, Streamable};
#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyNotImplementedError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;
use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign, Neg, SubAssign};

// we use the augmented scheme
pub(crate) const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";

#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(name = "G2Element"),
    derive(chia_py_streamable_macro::PyStreamable)
)]
#[derive(Clone, Default)]
pub struct Signature(pub(crate) blst_p2);

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Signature {
    fn arbitrary(_u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // placeholder
        Ok(Self::default())
    }
}

impl Signature {
    pub fn from_bytes_unchecked(buf: &[u8; 96]) -> Result<Self> {
        let p2 = unsafe {
            let mut p2_affine = MaybeUninit::<blst_p2_affine>::uninit();
            let ret = blst_p2_uncompress(p2_affine.as_mut_ptr(), buf.as_ptr());
            if ret != BLST_ERROR::BLST_SUCCESS {
                return Err(Error::InvalidSignature(ret));
            }
            let mut p2 = MaybeUninit::<blst_p2>::uninit();
            blst_p2_from_affine(p2.as_mut_ptr(), &p2_affine.assume_init());
            p2.assume_init()
        };
        Ok(Self(p2))
    }

    pub fn from_bytes(buf: &[u8; 96]) -> Result<Self> {
        let ret = Self::from_bytes_unchecked(buf)?;
        if ret.is_valid() {
            Ok(ret)
        } else {
            Err(Error::InvalidSignature(BLST_ERROR::BLST_POINT_NOT_ON_CURVE))
        }
    }

    pub fn from_uncompressed(buf: &[u8; 192]) -> Result<Self> {
        let p2 = unsafe {
            let mut p2_affine = MaybeUninit::<blst_p2_affine>::uninit();
            let ret = blst_p2_deserialize(p2_affine.as_mut_ptr(), buf.as_ptr());
            if ret != BLST_ERROR::BLST_SUCCESS {
                return Err(Error::InvalidSignature(ret));
            }
            let mut p2 = MaybeUninit::<blst_p2>::uninit();
            blst_p2_from_affine(p2.as_mut_ptr(), &p2_affine.assume_init());
            p2.assume_init()
        };
        Ok(Self(p2))
    }

    pub fn to_bytes(&self) -> [u8; 96] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; 96]>::uninit();
            blst_p2_compress(bytes.as_mut_ptr().cast::<u8>(), &self.0);
            bytes.assume_init()
        }
    }

    pub fn generator() -> Self {
        unsafe { Self(*blst_p2_generator()) }
    }

    pub fn aggregate(&mut self, sig: &Signature) {
        unsafe {
            blst_p2_add_or_double(&mut self.0, &self.0, &sig.0);
        }
    }

    pub fn is_valid(&self) -> bool {
        // Infinity was considered a valid G2Element in older Relic versions
        // For historical compatibililty this behavior is maintained.
        unsafe { blst_p2_is_inf(&self.0) || blst_p2_in_g2(&self.0) }
    }

    pub fn negate(&mut self) {
        unsafe {
            blst_p2_cneg(&mut self.0, true);
        }
    }

    pub fn scalar_multiply(&mut self, int_bytes: &[u8]) {
        unsafe {
            let mut scalar = MaybeUninit::<blst_scalar>::uninit();
            blst_scalar_from_be_bytes(scalar.as_mut_ptr(), int_bytes.as_ptr(), int_bytes.len());
            blst_p2_mult(&mut self.0, &self.0, scalar.as_ptr().cast::<u8>(), 256);
        }
    }

    pub fn pair(&self, other: &PublicKey) -> GTElement {
        let ans = unsafe {
            let mut ans = MaybeUninit::<blst_fp12>::uninit();
            let mut aff1 = MaybeUninit::<blst_p1_affine>::uninit();
            let mut aff2 = MaybeUninit::<blst_p2_affine>::uninit();

            blst_p1_to_affine(aff1.as_mut_ptr(), &other.0);
            blst_p2_to_affine(aff2.as_mut_ptr(), &self.0);

            blst_miller_loop(ans.as_mut_ptr(), &aff2.assume_init(), &aff1.assume_init());
            blst_final_exp(ans.as_mut_ptr(), ans.as_ptr());
            ans.assume_init()
        };
        GTElement(ans)
    }
}

impl Streamable for Signature {
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
        let input = read_bytes(input, 96)?.try_into().unwrap();
        if TRUSTED {
            Ok(Self::from_bytes_unchecked(input)?)
        } else {
            Ok(Self::from_bytes(input)?)
        }
    }
}

impl PartialEq for Signature {
    fn eq(&self, other: &Self) -> bool {
        unsafe { blst_p2_is_equal(&self.0, &other.0) }
    }
}
impl Eq for Signature {}

impl Hash for Signature {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.to_bytes());
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!(
            "<G2Element {}>",
            &hex::encode(self.to_bytes())
        ))
    }
}

impl AddAssign<&Signature> for Signature {
    fn add_assign(&mut self, rhs: &Signature) {
        unsafe {
            blst_p2_add_or_double(&mut self.0, &self.0, &rhs.0);
        }
    }
}

impl Neg for Signature {
    type Output = Signature;
    fn neg(mut self) -> Self::Output {
        self.negate();
        self
    }
}

impl Neg for &Signature {
    type Output = Signature;
    fn neg(self) -> Self::Output {
        let mut ret = self.clone();
        ret.negate();
        ret
    }
}

impl SubAssign<&Signature> for Signature {
    fn sub_assign(&mut self, rhs: &Signature) {
        unsafe {
            let mut neg = rhs.clone();
            blst_p2_cneg(&mut neg.0, true);
            blst_p2_add_or_double(&mut self.0, &self.0, &neg.0);
        }
    }
}

impl Add<&Signature> for Signature {
    type Output = Signature;
    fn add(mut self, rhs: &Signature) -> Signature {
        unsafe {
            blst_p2_add_or_double(&mut self.0, &self.0, &rhs.0);
            self
        }
    }
}

impl Add<&Signature> for &Signature {
    type Output = Signature;
    fn add(self, rhs: &Signature) -> Signature {
        let p1 = unsafe {
            let mut ret = MaybeUninit::<blst_p2>::uninit();
            blst_p2_add_or_double(ret.as_mut_ptr(), &self.0, &rhs.0);
            ret.assume_init()
        };
        Signature(p1)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        chia_serde::ser_bytes(&self.to_bytes(), serializer, true)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::from_bytes(&chia_serde::de_bytes(deserializer)?).map_err(serde::de::Error::custom)
    }
}

// validate a series of public keys (G1 points) and G2 points. These points are
// paired and the resulting GT points are multiplied. If the resulting GT point
// is the identity, the function returns true, otherwise false. To validate an
// aggregate signature, include the G1 generator and the signature as one of the
// pairs.
pub fn aggregate_pairing<G1: Borrow<PublicKey>, G2: Borrow<Signature>, I>(data: I) -> bool
where
    I: IntoIterator<Item = (G1, G2)>,
{
    let mut data = data.into_iter().peekable();
    if data.peek().is_none() {
        return true;
    }

    let mut v: Vec<u64> = vec![0; unsafe { blst_pairing_sizeof() } / 8];
    let ctx = unsafe {
        let ctx = v.as_mut_slice().as_mut_ptr().cast::<blst_pairing>();
        blst_pairing_init(
            ctx,
            true, // hash
            DST.as_ptr(),
            DST.len(),
        );
        ctx
    };

    for (g1, g2) in data {
        if !g1.borrow().is_valid() {
            return false;
        }
        if !g2.borrow().is_valid() {
            return false;
        }

        let g1_affine = unsafe {
            let mut g1_affine = MaybeUninit::<blst_p1_affine>::uninit();
            blst_p1_to_affine(g1_affine.as_mut_ptr(), &g1.borrow().0);
            g1_affine.assume_init()
        };

        let g2_affine = unsafe {
            let mut g2_affine = MaybeUninit::<blst_p2_affine>::uninit();
            blst_p2_to_affine(g2_affine.as_mut_ptr(), &g2.borrow().0);
            g2_affine.assume_init()
        };

        unsafe {
            blst_pairing_raw_aggregate(ctx, &g2_affine, &g1_affine);
        }
    }

    unsafe {
        blst_pairing_commit(ctx);
        blst_pairing_finalverify(ctx, std::ptr::null())
    }
}

pub fn hash_to_g2(msg: &[u8]) -> Signature {
    hash_to_g2_with_dst(msg, DST)
}

pub fn hash_to_g2_with_dst(msg: &[u8], dst: &[u8]) -> Signature {
    let p2 = unsafe {
        let mut p2 = MaybeUninit::<blst_p2>::uninit();
        blst_hash_to_g2(
            p2.as_mut_ptr(),
            msg.as_ptr(),
            msg.len(),
            dst.as_ptr(),
            dst.len(),
            std::ptr::null(),
            0,
        );
        p2.assume_init()
    };
    Signature(p2)
}

// aggregate the signatures into a single one. It can then be validated using
// aggregate_verify()
pub fn aggregate<Sig: Borrow<Signature>, I>(sigs: I) -> Signature
where
    I: IntoIterator<Item = Sig>,
{
    let mut ret = Signature::default();

    for s in sigs {
        ret.aggregate(s.borrow());
    }
    ret
}

// verify a signature given a single public key and message using the augmented
// scheme, i.e. the public key is pre-pended to the message before hashed to G2.
pub fn verify<Msg: AsRef<[u8]>>(sig: &Signature, key: &PublicKey, msg: Msg) -> bool {
    unsafe {
        let mut pubkey_affine = MaybeUninit::<blst_p1_affine>::uninit();
        let mut sig_affine = MaybeUninit::<blst_p2_affine>::uninit();

        blst_p1_to_affine(pubkey_affine.as_mut_ptr(), &key.0);
        blst_p2_to_affine(sig_affine.as_mut_ptr(), &sig.0);

        let mut augmented_msg = key.to_bytes().to_vec();
        augmented_msg.extend_from_slice(msg.as_ref());

        let err = blst_core_verify_pk_in_g1(
            &pubkey_affine.assume_init(),
            &sig_affine.assume_init(),
            true, // hash
            augmented_msg.as_ptr(),
            augmented_msg.len(),
            DST.as_ptr(),
            DST.len(),
            std::ptr::null(),
            0,
        );

        err == BLST_ERROR::BLST_SUCCESS
    }
}

// verify an aggregate signature given all public keys and messages.
// Messages will been augmented with the public key.
// returns true if the signature is valid.
pub fn aggregate_verify<Pk: Borrow<PublicKey>, Msg: Borrow<[u8]>, I>(
    sig: &Signature,
    data: I,
) -> bool
where
    I: IntoIterator<Item = (Pk, Msg)>,
{
    if !sig.is_valid() {
        return false;
    }

    let mut data = data.into_iter().peekable();
    if data.peek().is_none() {
        return *sig == Signature::default();
    }

    let sig_gt = unsafe {
        let mut sig_affine = MaybeUninit::<blst_p2_affine>::uninit();
        let mut sig_gt = MaybeUninit::<blst_fp12>::uninit();
        blst_p2_to_affine(sig_affine.as_mut_ptr(), &sig.0);
        blst_aggregated_in_g2(sig_gt.as_mut_ptr(), sig_affine.as_ptr());
        sig_gt.assume_init()
    };

    let mut v: Vec<u64> = vec![0; unsafe { blst_pairing_sizeof() } / 8];
    let ctx = unsafe {
        let ctx = v.as_mut_ptr().cast::<blst_pairing>();
        blst_pairing_init(
            ctx,
            true, // hash
            DST.as_ptr(),
            DST.len(),
        );
        ctx
    };

    let mut aug_msg = Vec::<u8>::new();
    for (pk, msg) in data {
        if !pk.borrow().is_valid() {
            return false;
        }

        let pk_affine = unsafe {
            let mut pk_affine = MaybeUninit::<blst_p1_affine>::uninit();
            blst_p1_to_affine(pk_affine.as_mut_ptr(), &pk.borrow().0);
            pk_affine.assume_init()
        };

        aug_msg.clear();
        aug_msg.extend_from_slice(&pk.borrow().to_bytes());
        aug_msg.extend_from_slice(msg.borrow());

        let err = unsafe {
            blst_pairing_aggregate_pk_in_g1(
                ctx,
                &pk_affine,
                std::ptr::null(),
                aug_msg.as_ptr(),
                aug_msg.len(),
                std::ptr::null(),
                0,
            )
        };

        if err != BLST_ERROR::BLST_SUCCESS {
            return false;
        }
    }

    unsafe {
        blst_pairing_commit(ctx);
        blst_pairing_finalverify(ctx, &sig_gt)
    }
}

// verify an aggregate signature by pre-paired public keys and messages.
// Messages having been augmented and hashed to G2 and then paired with the G1
// public key.
// returns true if the signature is valid.
pub fn aggregate_verify_gt<Gt: Borrow<GTElement>, I>(sig: &Signature, data: I) -> bool
where
    I: IntoIterator<Item = Gt>,
{
    if !sig.is_valid() {
        return false;
    }

    let mut data = data.into_iter();
    let Some(agg) = data.next() else {
        return *sig == Signature::default();
    };

    let mut agg = agg.borrow().clone();
    for gt in data {
        agg *= gt.borrow();
    }

    agg == sig.pair(&PublicKey::generator())
}

// Signs msg using sk without augmenting the message with the public key. This
// function is used when the caller augments the message with some other public
// key
pub fn sign_raw<Msg: AsRef<[u8]>>(sk: &SecretKey, msg: Msg) -> Signature {
    let p2 = unsafe {
        let mut p2 = MaybeUninit::<blst_p2>::uninit();
        blst_hash_to_g2(
            p2.as_mut_ptr(),
            msg.as_ref().as_ptr(),
            msg.as_ref().len(),
            DST.as_ptr(),
            DST.len(),
            std::ptr::null(),
            0,
        );
        blst_sign_pk_in_g1(p2.as_mut_ptr(), p2.as_ptr(), &sk.0);
        p2.assume_init()
    };
    Signature(p2)
}

// Signs msg using sk using the augmented scheme, meaning the public key is
// pre-pended to msg befire signing.
pub fn sign<Msg: AsRef<[u8]>>(sk: &SecretKey, msg: Msg) -> Signature {
    let mut aug_msg = sk.public_key().to_bytes().to_vec();
    aug_msg.extend_from_slice(msg.as_ref());
    sign_raw(sk, aug_msg)
}

#[cfg(feature = "py-bindings")]
#[pyo3::pymethods]
impl Signature {
    #[classattr]
    pub const SIZE: usize = 96;

    #[new]
    pub fn init() -> Self {
        Self::default()
    }

    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "Signature does not support from_parent().",
        ))
    }

    #[pyo3(name = "pair")]
    pub fn py_pair(&self, other: &PublicKey) -> GTElement {
        self.pair(other)
    }

    #[staticmethod]
    #[pyo3(name = "generator")]
    pub fn py_generator() -> Self {
        Self::generator()
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

    impl ToJsonDict for Signature {
        fn to_json_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
            let bytes = self.to_bytes();
            Ok(("0x".to_string() + &hex::encode(bytes))
                .into_pyobject(py)?
                .into_any()
                .unbind())
        }
    }

    impl FromJsonDict for Signature {
        fn from_json_dict(o: &Bound<'_, PyAny>) -> PyResult<Self> {
            Ok(Self::from_bytes(
                parse_hex_string(o, 96, "Signature")?
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
    use rstest::rstest;

    #[test]
    fn test_from_bytes() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 96];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            // just any random bytes are not a valid signature and should fail
            match Signature::from_bytes(&data) {
                Err(Error::InvalidSignature(err)) => {
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

    #[test]
    fn test_default_is_valid() {
        let sig = Signature::default();
        assert!(sig.is_valid());
    }

    #[test]
    fn test_infinity_is_valid() {
        let mut data = [0u8; 96];
        data[0] = 0xc0;
        let sig = Signature::from_bytes(&data).unwrap();
        assert!(sig.is_valid());
    }

    #[test]
    fn test_is_valid() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        let msg = [0u8; 32];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let sig = sign(&sk, msg);
            assert!(sig.is_valid());
        }
    }

    #[test]
    fn test_roundtrip() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        let mut msg = [0u8; 32];
        rng.fill(msg.as_mut_slice());
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let sig = sign(&sk, msg);
            let bytes = sig.to_bytes();
            let sig2 = Signature::from_bytes(&bytes).unwrap();
            assert_eq!(sig, sig2);
        }
    }

    #[test]
    fn test_random_verify() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        let mut msg = [0u8; 32];
        rng.fill(msg.as_mut_slice());
        for _i in 0..20 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let pk = sk.public_key();
            let sig = sign(&sk, msg);
            assert!(verify(&sig, &pk, msg));

            let bytes = sig.to_bytes();
            let sig2 = Signature::from_bytes(&bytes).unwrap();
            assert!(verify(&sig2, &pk, msg));
        }
    }

    #[test]
    fn test_verify() {
        // test case from:
        // from blspy import PrivateKey
        // from blspy import AugSchemeMPL
        // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
        // data = b"foobar"
        // print(AugSchemeMPL.sign(sk, data))
        let msg = b"foobar";
        let sk = SecretKey::from_bytes(
            &<[u8; 32]>::from_hex(
                "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb",
            )
            .unwrap(),
        )
        .unwrap();

        let sig = sign(&sk, msg);
        assert!(verify(&sig, &sk.public_key(), msg));

        assert_eq!(sig.to_bytes(), <[u8; 96]>::from_hex("b45825c0ee7759945c0189b4c38b7e54231ebadc83a851bec3bb7cf954a124ae0cc8e8e5146558332ea152f63bf8846e04826185ef60e817f271f8d500126561319203f9acb95809ed20c193757233454be1562a5870570941a84605bd2c9c9a").unwrap());
    }

    fn aug_msg_to_g2(pk: &PublicKey, msg: &[u8]) -> Signature {
        let mut augmented = pk.to_bytes().to_vec();
        augmented.extend_from_slice(msg);
        hash_to_g2(augmented.as_slice())
    }

    #[test]
    fn test_aggregate_signature() {
        // from blspy import PrivateKey
        // from blspy import AugSchemeMPL
        // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
        // data = b"foobar"
        // sk0 = AugSchemeMPL.derive_child_sk(sk, 0)
        // sk1 = AugSchemeMPL.derive_child_sk(sk, 1)
        // sk2 = AugSchemeMPL.derive_child_sk(sk, 2)
        // sk3 = AugSchemeMPL.derive_child_sk(sk, 3)

        // sig0 = AugSchemeMPL.sign(sk0, data)
        // sig1 = AugSchemeMPL.sign(sk1, data)
        // sig2 = AugSchemeMPL.sign(sk2, data)
        // sig3 = AugSchemeMPL.sign(sk3, data)

        // agg = AugSchemeMPL.aggregate([sig0, sig1, sig2, sig3])

        // 87bce2c588f4257e2792d929834548c7d3af679272cb4f8e1d24cf4bf584dd287aa1d9f5e53a86f288190db45e1d100d0a5e936079a66a709b5f35394cf7d52f49dd963284cb5241055d54f8cf48f61bc1037d21cae6c025a7ea5e9f4d289a18

        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        let msg = b"foobar";
        let mut agg1 = Signature::default();
        let mut agg2 = Signature::default();
        let mut sigs = Vec::<Signature>::new();
        let mut data = Vec::<(PublicKey, &[u8])>::new();
        let mut pairs = Vec::<(PublicKey, Signature)>::new();
        for idx in 0..4 {
            let derived = sk.derive_hardened(idx as u32);
            let pk = derived.public_key();
            data.push((pk, msg));
            let sig = sign(&derived, msg);
            agg1.aggregate(&sig);
            agg2 += &sig;
            sigs.push(sig);
            pairs.push((pk, aug_msg_to_g2(&pk, msg)));
        }
        let agg3 = aggregate(&sigs);
        let agg4 = &sigs[0] + &sigs[1] + &sigs[2] + &sigs[3];

        assert_eq!(agg1.to_bytes(), <[u8; 96]>::from_hex("87bce2c588f4257e2792d929834548c7d3af679272cb4f8e1d24cf4bf584dd287aa1d9f5e53a86f288190db45e1d100d0a5e936079a66a709b5f35394cf7d52f49dd963284cb5241055d54f8cf48f61bc1037d21cae6c025a7ea5e9f4d289a18").unwrap());
        assert_eq!(agg1, agg2);
        assert_eq!(agg1, agg3);
        assert_eq!(agg1, agg4);

        // ensure the aggregate signature verifies OK
        assert!(aggregate_verify(&agg1, data.clone()));
        assert!(aggregate_verify(&agg2, data.clone()));
        assert!(aggregate_verify(&agg3, data.clone()));
        assert!(aggregate_verify(&agg4, data.clone()));

        pairs.push((-PublicKey::generator(), agg1));
        assert!(aggregate_pairing(pairs.clone()));
        // order does not matter
        assert!(aggregate_pairing(pairs.into_iter().rev()));
    }

    #[rstest]
    fn test_aggregate_gt_signature(#[values(0, 1, 2, 3, 4, 5, 100)] num_keys: usize) {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        let msg = b"foobar";
        let mut agg = Signature::default();
        let mut gts = Vec::<GTElement>::new();
        let mut pks = Vec::<PublicKey>::new();
        for idx in 0..num_keys {
            let derived = sk.derive_hardened(idx as u32);
            let pk = derived.public_key();
            let sig = sign(&derived, msg);
            agg.aggregate(&sig);
            gts.push(aug_msg_to_g2(&pk, msg).pair(&pk));
            pks.push(pk);
        }

        assert!(aggregate_verify_gt(&agg, &gts));
        assert!(aggregate_verify(&agg, pks.iter().map(|pk| (pk, &msg[..]))));

        // the order of the GTElements does not matter
        for _ in 0..num_keys {
            gts.rotate_right(1);
            pks.rotate_right(1);
            assert!(aggregate_verify_gt(&agg, &gts));
            assert!(aggregate_verify(&agg, pks.iter().map(|pk| (pk, &msg[..]))));
        }
        for _ in 0..num_keys {
            gts.rotate_right(1);
            pks.rotate_right(1);
            assert!(!aggregate_verify_gt(&agg, &gts[1..]));
            assert!(!aggregate_verify(
                &agg,
                pks[1..].iter().map(|pk| (pk, &msg[..]))
            ));
        }
    }

    #[test]
    fn test_aggregate_duplicate_signature() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        let msg = b"foobar";
        let mut agg = Signature::default();
        let mut data = Vec::<(PublicKey, &[u8])>::new();
        let mut pairs = Vec::<(PublicKey, Signature)>::new();
        for _idx in 0..2 {
            let pk = sk.public_key();
            data.push((pk, msg));
            agg.aggregate(&sign(&sk, *msg));

            pairs.push((pk, aug_msg_to_g2(&pk, msg)));
        }

        assert_eq!(agg.to_bytes(), <[u8; 96]>::from_hex("a1cca6540a4a06d096cb5b5fc76af5fd099476e70b623b8c6e4cf02ffde94fc0f75f4e17c67a9e350940893306798a3519368b02dc3464b7270ea4ca233cfa85a38da9e25c9314e81270b54d1e773a2ec5c3e14c62dac7abdebe52f4688310d3").unwrap());

        assert!(aggregate_verify(&agg, data));

        pairs.push((-PublicKey::generator(), agg));
        assert!(aggregate_pairing(pairs.clone()));
        // order does not matter
        assert!(aggregate_pairing(pairs.into_iter().rev()));
    }

    #[cfg(test)]
    fn random_sk<R: Rng>(rng: &mut R) -> SecretKey {
        let mut data = [0u8; 64];
        rng.fill(data.as_mut_slice());
        SecretKey::from_seed(&data)
    }

    #[test]
    fn test_aggregate_signature_separate_msg() {
        let mut rng = StdRng::seed_from_u64(1337);
        let sk = [random_sk(&mut rng), random_sk(&mut rng)];
        let pk = [sk[0].public_key(), sk[1].public_key()];
        let msg: [&'static [u8]; 2] = [b"foo", b"foobar"];
        let sig = [sign(&sk[0], msg[0]), sign(&sk[1], msg[1])];
        let mut agg = Signature::default();
        agg.aggregate(&sig[0]);
        agg.aggregate(&sig[1]);

        assert!(aggregate_verify(&agg, pk.iter().zip(msg)));
        // order does not matter
        assert!(aggregate_verify(&agg, pk.iter().zip(msg).rev()));
    }

    #[test]
    fn test_aggregate_signature_identity() {
        // when verifying 0 messages, an identity signature is considered valid
        let empty = Vec::<(PublicKey, &[u8])>::new();
        assert!(aggregate_verify(&Signature::default(), empty));

        let pairs = vec![(-PublicKey::generator(), Signature::default())];
        assert!(aggregate_pairing(pairs));
    }

    #[test]
    fn test_invalid_aggregate_signature() {
        let mut rng = StdRng::seed_from_u64(1337);
        let sk = [random_sk(&mut rng), random_sk(&mut rng)];
        let pk = [sk[0].public_key(), sk[1].public_key()];
        let msg: [&'static [u8]; 2] = [b"foo", b"foobar"];
        let sig = [sign(&sk[0], msg[0]), sign(&sk[1], msg[1])];
        let g2s = [aug_msg_to_g2(&pk[0], msg[0]), aug_msg_to_g2(&pk[1], msg[1])];
        let mut agg = Signature::default();
        agg.aggregate(&sig[0]);
        agg.aggregate(&sig[1]);

        assert!(!aggregate_verify(&agg, [(&pk[0], msg[0])]));
        assert!(!aggregate_verify(&agg, [(&pk[1], msg[1])]));
        // public keys mixed with the wrong message
        assert!(!aggregate_verify(
            &agg,
            [(&pk[0], msg[1]), (&pk[1], msg[0])]
        ));
        assert!(!aggregate_verify(
            &agg,
            [(&pk[1], msg[0]), (&pk[0], msg[1])]
        ));

        let gen_sig = (&-PublicKey::generator(), agg);
        assert!(!aggregate_pairing([
            (&pk[0], g2s[0].clone()),
            gen_sig.clone()
        ]));
        assert!(!aggregate_pairing([
            (&pk[1], g2s[1].clone()),
            gen_sig.clone()
        ]));
        // public keys mixed with the wrong message
        assert!(!aggregate_pairing([
            (&pk[0], g2s[1].clone()),
            (&pk[1], g2s[0].clone()),
            gen_sig.clone()
        ]));
        assert!(!aggregate_pairing([
            (&pk[1], g2s[0].clone()),
            (&pk[0], g2s[1].clone()),
            gen_sig.clone()
        ]));
    }

    #[test]
    fn test_vector_2_aggregate_of_aggregates() {
        // test case from: bls-signatures/src/test.cpp
        // "Chia test vector 2 (Augmented, aggregate of aggregates)"
        let message1 = [1_u8, 2, 3, 40];
        let message2 = [5_u8, 6, 70, 201];
        let message3 = [9_u8, 10, 11, 12, 13];
        let message4 = [15_u8, 63, 244, 92, 0, 1];

        let sk1 = SecretKey::from_seed(&[2_u8; 32]);
        let sk2 = SecretKey::from_seed(&[3_u8; 32]);

        let pk1 = sk1.public_key();
        let pk2 = sk2.public_key();

        let sig1 = sign(&sk1, message1);
        let sig2 = sign(&sk2, message2);
        let sig3 = sign(&sk2, message1);
        let sig4 = sign(&sk1, message3);
        let sig5 = sign(&sk1, message1);
        let sig6 = sign(&sk1, message4);

        let agg_sig_l = aggregate([sig1, sig2]);
        let agg_sig_r = aggregate([sig3, sig4, sig5]);
        let aggsig = aggregate([agg_sig_l, agg_sig_r, sig6]);

        assert!(aggregate_verify(
            &aggsig,
            [
                (&pk1, message1.as_ref()),
                (&pk2, message2.as_ref()),
                (&pk2, message1.as_ref()),
                (&pk1, message3.as_ref()),
                (&pk1, message1.as_ref()),
                (&pk1, message4.as_ref())
            ]
        ));

        assert_eq!(
            aggsig.to_bytes(),
            <[u8; 96]>::from_hex(
                "a1d5360dcb418d33b29b90b912b4accde535cf0e52caf467a005dc632d9f7af44b6c4e9acd4\
            6eac218b28cdb07a3e3bc087df1cd1e3213aa4e11322a3ff3847bbba0b2fd19ddc25ca964871\
            997b9bceeab37a4c2565876da19382ea32a962200"
            )
            .unwrap()
        );
    }

    #[test]
    fn test_signature_zero_key() {
        // test case from: bls-signatures/src/test.cpp
        // "Should sign with the zero key"
        let sk = SecretKey::from_bytes(&[0; 32]).unwrap();
        assert_eq!(sign(&sk, [1_u8, 2, 3]), Signature::default());
    }

    #[test]
    fn test_aggregate_many_g2_elements_diff_message() {
        // test case from: bls-signatures/src/test.cpp
        // "Should Aug aggregate many G2Elements, diff message"

        let mut rng = StdRng::seed_from_u64(1337);

        let mut pairs = Vec::<(PublicKey, Vec<u8>)>::new();
        let mut sigs = Vec::<Signature>::new();

        for i in 0..80 {
            let message = vec![0_u8, 100, 2, 45, 64, 12, 12, 63, i];
            let sk = random_sk(&mut rng);
            let sig = sign(&sk, &message);
            pairs.push((sk.public_key(), message));
            sigs.push(sig);
        }

        let aggsig = aggregate(sigs);

        assert!(aggregate_verify(&aggsig, pairs));
    }

    #[test]
    fn test_aggregate_identity() {
        // test case from: bls-signatures/src/test.cpp
        // "Aggregate Verification of zero items with infinity should pass"
        let sig = Signature::default();
        let aggsig = aggregate([&sig]);
        assert_eq!(aggsig, sig);
        assert_eq!(aggsig, Signature::default());

        let pairs: [(&PublicKey, &[u8]); 0] = [];
        assert!(aggregate_verify(&aggsig, pairs));
    }

    #[test]
    fn test_aggregate_multiple_levels_degenerate() {
        // test case from: bls-signatures/src/test.cpp
        // "Should aggregate with multiple levels, degenerate"

        let mut rng = StdRng::seed_from_u64(1337);

        let message1 = [100_u8, 2, 254, 88, 90, 45, 23];
        let sk1 = random_sk(&mut rng);
        let pk1 = sk1.public_key();
        let mut agg_sig = sign(&sk1, message1);
        let mut pairs: Vec<(PublicKey, &[u8])> = vec![(pk1, &message1)];

        for _i in 0..10 {
            let sk = random_sk(&mut rng);
            let pk = sk.public_key();
            pairs.push((pk, &message1));
            let sig = sign(&sk, message1);
            agg_sig.aggregate(&sig);
        }
        assert!(aggregate_verify(&agg_sig, pairs));
    }

    #[test]
    fn test_aggregate_multiple_levels_different_messages() {
        // test case from: bls-signatures/src/test.cpp
        // "Should aggregate with multiple levels, different messages"

        let mut rng = StdRng::seed_from_u64(1337);

        let message1 = [100_u8, 2, 254, 88, 90, 45, 23];
        let message2 = [192_u8, 29, 2, 0, 0, 45, 23];
        let message3 = [52_u8, 29, 2, 0, 0, 45, 102];
        let message4 = [99_u8, 29, 2, 0, 0, 45, 222];

        let sk1 = random_sk(&mut rng);
        let sk2 = random_sk(&mut rng);

        let pk1 = sk1.public_key();
        let pk2 = sk2.public_key();

        let sig1 = sign(&sk1, message1);
        let sig2 = sign(&sk2, message2);
        let sig3 = sign(&sk2, message3);
        let sig4 = sign(&sk1, message4);

        let agg_sig_l = aggregate([sig1, sig2]);
        let agg_sig_r = aggregate([sig3, sig4]);
        let agg_sig = aggregate([agg_sig_l, agg_sig_r]);

        let all_pairs: [(&PublicKey, &[u8]); 4] = [
            (&pk1, &message1),
            (&pk2, &message2),
            (&pk2, &message3),
            (&pk1, &message4),
        ];
        assert!(aggregate_verify(&agg_sig, all_pairs));
    }

    #[test]
    fn test_aug_scheme() {
        // test case from: bls-signatures/src/test.cpp
        // "Aug Scheme"

        let msg1 = [7_u8, 8, 9];
        let msg2 = [10_u8, 11, 12];

        let sk1 = SecretKey::from_seed(&[4_u8; 32]);
        let pk1 = sk1.public_key();
        let pk1v = pk1.to_bytes();
        let sig1 = sign(&sk1, msg1);
        let sig1v = sig1.to_bytes();

        assert!(verify(&sig1, &pk1, msg1));
        assert!(verify(
            &Signature::from_bytes(&sig1v).unwrap(),
            &PublicKey::from_bytes(&pk1v).unwrap(),
            msg1
        ));

        let sk2 = SecretKey::from_seed(&[5_u8; 32]);
        let pk2 = sk2.public_key();
        let pk2v = pk2.to_bytes();
        let sig2 = sign(&sk2, msg2);
        let sig2v = sig2.to_bytes();

        assert!(verify(&sig2, &pk2, msg2));
        assert!(verify(
            &Signature::from_bytes(&sig2v).unwrap(),
            &PublicKey::from_bytes(&pk2v).unwrap(),
            msg2
        ));

        // Wrong G2Element
        assert!(!verify(&sig2, &pk1, msg1));
        assert!(!verify(
            &Signature::from_bytes(&sig2v).unwrap(),
            &PublicKey::from_bytes(&pk1v).unwrap(),
            msg1
        ));
        // Wrong msg
        assert!(!verify(&sig1, &pk1, msg2));
        assert!(!verify(
            &Signature::from_bytes(&sig1v).unwrap(),
            &PublicKey::from_bytes(&pk1v).unwrap(),
            msg2
        ));
        // Wrong pk
        assert!(!verify(&sig1, &pk2, msg1));
        assert!(!verify(
            &Signature::from_bytes(&sig1v).unwrap(),
            &PublicKey::from_bytes(&pk2v).unwrap(),
            msg1
        ));

        let aggsig = aggregate([sig1, sig2]);
        let aggsigv = aggsig.to_bytes();
        let pairs: [(&PublicKey, &[u8]); 2] = [(&pk1, &msg1), (&pk2, &msg2)];
        assert!(aggregate_verify(&aggsig, pairs));
        assert!(aggregate_verify(
            &Signature::from_bytes(&aggsigv).unwrap(),
            pairs
        ));
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
        let sig1 = sign(&sk, [0, 1, 2]);
        let sig2 = sign(&sk, [0, 1, 2, 3]);

        assert!(hash(sig1) != hash(sig2));
        assert_eq!(hash(sign(&sk, [0, 1, 2])), hash(sign(&sk, [0, 1, 2])));
    }

    #[test]
    fn test_debug() {
        let mut data = [0u8; 96];
        data[0] = 0xc0;
        let sig = Signature::from_bytes(&data).unwrap();
        assert_eq!(
            format!("{sig:?}"),
            format!("<G2Element {}>", hex::encode(data))
        );
    }

    #[test]
    fn test_generator() {
        assert_eq!(
        hex::encode(Signature::generator().to_bytes()),
        "93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8"
        );
    }

    // test cases from zksnark test in chia_rs
    #[rstest]
    #[case("0a7ecb9c6d6f0af8d922c9b348d686f7f827c5f5d7a53036e5dd6c4cfe088806375d730251df57c03b0eaa41ca2a9cc51817cfd6118c065e9b337e42a6b66621e2ffa79f576ae57dcb4916459b0131d42383b790a4f60c5aeb339b61a78d85a808b73e0701084dc16b5d7aa8c2f5385f83a217bc29934d0d02c51365410232e3c0288438e3110aa6e8cdef7bd32c46d60d0104952aaa0f0545cbe1548b70eed8b543ce19ede34cc51a387d092221417db0253f4651666b17303e225eac706107", "8a7ecb9c6d6f0af8d922c9b348d686f7f827c5f5d7a53036e5dd6c4cfe088806375d730251df57c03b0eaa41ca2a9cc51817cfd6118c065e9b337e42a6b66621e2ffa79f576ae57dcb4916459b0131d42383b790a4f60c5aeb339b61a78d85a8")]
    #[case("13e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb80606c4a02ea734cc32acd2b02bc28b99cb3e287e85a763af267492ab572e99ab3f370d275cec1da1aaa9075ff05f79be0ce5d527727d6e118cc9cdc6da2e351aadfd9baa8cbdd3a76d429a695160d12c923ac9cc3baca289e193548608b82801", "93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8")]
    #[case("140acf170629d78244fb753f05fb79578add9217add53996d5de7c3005880c0dea903f851d6be749ebfb81c9721871370ef60428444d76f4ff81515628a4eb63e72c3cd7651a23c4eca109d1d88fec5a53626b36c76407926f308366b5ded1b219a481d87c6f87a4021fa8aa32851874f01b3eb011f6ed69c7884717fb0f5239bdc7310c2bc287659cd4a93976deaac20f4a21f0b004c767be4a21f36861616a5399b3e27431dc8133f325603230eaf1debdce8077105ab46baafa4836842305", "b40acf170629d78244fb753f05fb79578add9217add53996d5de7c3005880c0dea903f851d6be749ebfb81c9721871370ef60428444d76f4ff81515628a4eb63e72c3cd7651a23c4eca109d1d88fec5a53626b36c76407926f308366b5ded1b2")]
    fn test_from_uncompressed(#[case] input: &str, #[case] expect: &str) {
        let input = hex::decode(input).unwrap();
        let g2 = Signature::from_uncompressed(input.as_slice().try_into().unwrap()).unwrap();
        let compressed = g2.to_bytes();
        assert_eq!(hex::encode(compressed), expect);
    }

    #[test]
    fn test_negate_roundtrip() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        let mut msg = [0u8; 32];
        rng.fill(msg.as_mut_slice());
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let g2 = sign(&sk, msg);

            let mut g2_neg = g2.clone();
            g2_neg.negate();
            assert!(g2_neg != g2);

            g2_neg.negate();
            assert!(g2_neg == g2);
        }
    }

    #[test]
    fn test_negate_infinity() {
        let g2 = Signature::default();
        let mut g2_neg = g2.clone();
        // negate on infinity is a no-op
        g2_neg.negate();
        assert!(g2_neg == g2);
    }

    #[test]
    fn test_negate() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        let mut msg = [0u8; 32];
        rng.fill(msg.as_mut_slice());
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let g2 = sign(&sk, msg);
            let mut g2_neg = g2.clone();
            g2_neg.negate();

            let mut g2_double = g2.clone();
            // adding the negative undoes adding the positive
            g2_double += &g2;
            assert!(g2_double != g2);
            g2_double += &g2_neg;
            assert!(g2_double == g2);
        }
    }

    #[test]
    fn test_scalar_multiply() {
        let mut rng = StdRng::seed_from_u64(1337);
        let mut data = [0u8; 32];
        let mut msg = [0u8; 32];
        rng.fill(msg.as_mut_slice());
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let mut g2 = sign(&sk, msg);
            let mut g2_double = g2.clone();
            g2_double += &g2;
            assert!(g2_double != g2);
            // scalar multiply by 2 is the same as adding oneself
            g2.scalar_multiply(&[2]);
            assert!(g2_double == g2);
        }
    }

    #[test]
    fn test_hash_to_g2_different_dst() {
        const DEFAULT_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";
        const CUSTOM_DST: &[u8] = b"foobar";

        let mut rng = StdRng::seed_from_u64(1337);
        let mut msg = [0u8; 32];
        for _i in 0..50 {
            rng.fill(&mut msg);
            let default_hash = hash_to_g2(&msg);
            assert_eq!(default_hash, hash_to_g2_with_dst(&msg, DEFAULT_DST));
            assert!(default_hash != hash_to_g2_with_dst(&msg, CUSTOM_DST));
        }
    }

    // test cases from clvm_rs
    #[rstest]
    #[case("abcdef0123456789", "92596412844e12c4733b5a6bfc5727cde4c20b345665d2de99de163266f3ba6a944c6c0fdd9d9fe57b9a4acb769bf3780456f8aab4cd41a70836dba57a5278a85fbd18eb96a2b56cfbda853186c9d190c43e63bc3e6a181aed692e97bbdb1944")]
    fn test_hash_to_g2(#[case] input: &str, #[case] expect: &str) {
        let g2 = hash_to_g2(input.as_bytes());
        assert_eq!(hex::encode(g2.to_bytes()), expect);
    }

    // test cases from clvm_rs
    #[rstest]
    #[case("abcdef0123456789", "BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_", "8ee1ff66094b8975401c86ad424076d97fed9c2025db5f9dfde6ed455c7bff34b55e96379c1f9ee3c173633587f425e50aed3e807c6c7cd7bed35d40542eee99891955b2ea5321ebde37172e2c01155138494c2d725b03c02765828679bf011e")]
    #[case("abcdef0123456789", "BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_", "92596412844e12c4733b5a6bfc5727cde4c20b345665d2de99de163266f3ba6a944c6c0fdd9d9fe57b9a4acb769bf3780456f8aab4cd41a70836dba57a5278a85fbd18eb96a2b56cfbda853186c9d190c43e63bc3e6a181aed692e97bbdb1944")]
    fn test_hash_to_g2_with_dst(#[case] input: &str, #[case] dst: &str, #[case] expect: &str) {
        let g2 = hash_to_g2_with_dst(input.as_bytes(), dst.as_bytes());
        assert_eq!(hex::encode(g2.to_bytes()), expect);
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
        let mut msg = [0u8; 10];
        for _i in 0..50 {
            rng.fill(data.as_mut_slice());
            rng.fill(msg.as_mut_slice());
            let sk = SecretKey::from_seed(&data);
            let sig = sign(&sk, msg);
            Python::with_gil(|py| {
                let string = sig.to_json_dict(py).expect("to_json_dict");
                let py_class = py.get_type::<Signature>();
                let sig2 = Signature::from_json_dict(&py_class, py, string.bind(py))
                    .unwrap()
                    .extract(py)
                    .unwrap();
                assert_eq!(sig, sig2);
            });
        }
    }

    #[rstest]
    #[case("0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0ff000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e", "Signature, invalid length 95 expected 96")]
    #[case("0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0ff000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f00", "Signature, invalid length 97 expected 96")]
    #[case("000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0ff000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e", "Signature, invalid length 95 expected 96")]
    #[case("000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0ff000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f00", "Signature, invalid length 97 expected 96")]
    #[case("00r102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0ff000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f", "invalid hex")]
    fn test_json_dict(#[case] input: &str, #[case] msg: &str) {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_class = py.get_type::<Signature>();
            let err = Signature::from_json_dict(
                &py_class,
                py,
                &input.to_string().into_pyobject(py).unwrap().into_any(),
            )
            .unwrap_err();
            assert_eq!(err.value(py).to_string(), msg.to_string());
        });
    }
}
