use crate::{Error, GTElement, PublicKey, Result, SecretKey};
use blst::*;
use chia_traits::{read_bytes, Streamable};
use clvm_traits::{FromClvm, ToClvm};
use clvmr::allocator::{Allocator, NodePtr, SExp};
use sha2::{Digest, Sha256};
use std::borrow::Borrow;
use std::convert::AsRef;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign};

#[cfg(feature = "py-bindings")]
use crate::public_key::parse_hex_string;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use chia_traits::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use chia_traits::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods, IntoPy, PyAny, PyObject, PyResult, Python};

// we use the augmented scheme
pub const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";

#[cfg_attr(
    feature = "py-bindings",
    pyclass(name = "G2Element"),
    derive(PyStreamable)
)]
#[derive(Clone)]
pub struct Signature(pub(crate) blst_p2);

impl Signature {
    pub fn from_bytes_unchecked(buf: &[u8; 96]) -> Result<Self> {
        let p2 = unsafe {
            let mut p2_affine = MaybeUninit::<blst_p2_affine>::uninit();
            let ret = blst_p2_uncompress(p2_affine.as_mut_ptr(), buf as *const u8);
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
        if !ret.is_valid() {
            Err(Error::InvalidSignature(BLST_ERROR::BLST_POINT_NOT_ON_CURVE))
        } else {
            Ok(ret)
        }
    }

    pub fn to_bytes(&self) -> [u8; 96] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; 96]>::uninit();
            blst_p2_compress(bytes.as_mut_ptr() as *mut u8, &self.0);
            bytes.assume_init()
        }
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

impl Default for Signature {
    fn default() -> Self {
        unsafe {
            let p2 = MaybeUninit::<blst_p2>::zeroed();
            Self(p2.assume_init())
        }
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

    fn parse(input: &mut Cursor<&[u8]>) -> chia_traits::chia_error::Result<Self> {
        Ok(Self::from_bytes(
            read_bytes(input, 96)?.try_into().unwrap(),
        )?)
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
        state.write(&self.to_bytes())
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&hex::encode(self.to_bytes()))
    }
}

impl AddAssign<&Signature> for Signature {
    fn add_assign(&mut self, rhs: &Signature) {
        unsafe {
            blst_p2_add_or_double(&mut self.0, &self.0, &rhs.0);
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

#[cfg(feature = "py-bindings")]
impl ToJsonDict for Signature {
    fn to_json_dict(&self, py: Python) -> pyo3::PyResult<PyObject> {
        let bytes = self.to_bytes();
        Ok(("0x".to_string() + &hex::encode(bytes)).into_py(py))
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for Signature {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        Ok(Self::from_bytes(
            parse_hex_string(o, 96, "Signature")?
                .as_slice()
                .try_into()
                .unwrap(),
        )?)
    }
}

impl FromClvm for Signature {
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

impl ToClvm for Signature {
    fn to_clvm(&self, a: &mut Allocator) -> clvm_traits::Result<NodePtr> {
        Ok(a.new_atom(&self.to_bytes())?)
    }
}

#[cfg(feature = "py-bindings")]
#[cfg_attr(feature = "py-bindings", pymethods)]
impl Signature {
    #[classattr]
    const SIZE: usize = 96;

    #[new]
    pub fn init() -> Self {
        Self::default()
    }

    #[staticmethod]
    #[pyo3(name = "from_bytes_unchecked")]
    pub fn py_from_bytes_unchecked(bytes: [u8; Self::SIZE]) -> Result<Signature> {
        Self::from_bytes_unchecked(&bytes)
    }

    #[pyo3(name = "pair")]
    pub fn py_pair(&self, other: &PublicKey) -> GTElement {
        self.pair(other)
    }

    #[staticmethod]
    pub fn generator() -> Self {
        unsafe { Self(*blst_p2_generator()) }
    }

    pub fn __repr__(&self) -> String {
        let bytes = self.to_bytes();
        format!("<G2Element {}>", &hex::encode(bytes))
    }

    pub fn __add__(&self, rhs: &Self) -> Self {
        self + rhs
    }

    pub fn __iadd__(&mut self, rhs: &Self) {
        *self += rhs;
    }
}

pub fn hash_to_g2(msg: &[u8]) -> Signature {
    let p2 = unsafe {
        let mut p2 = MaybeUninit::<blst_p2>::uninit();
        blst_hash_to_g2(
            p2.as_mut_ptr(),
            msg.as_ptr(),
            msg.len(),
            DST.as_ptr(),
            DST.len(),
            std::ptr::null(),
            0,
        );
        p2.assume_init()
    };
    Signature(p2)
}

pub fn aggregate<Sig: Borrow<Signature>, I>(sigs: I) -> Signature
where
    I: IntoIterator<Item = Sig>,
{
    let mut ret = Signature::default();

    for s in sigs.into_iter() {
        ret.aggregate(s.borrow());
    }
    ret
}

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
        let ctx = v.as_mut_slice().as_mut_ptr() as *mut blst_pairing;
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

pub fn sign<Msg: AsRef<[u8]>>(sk: &SecretKey, msg: Msg) -> Signature {
    let mut aug_msg = sk.public_key().to_bytes().to_vec();
    aug_msg.extend_from_slice(msg.as_ref());
    sign_raw(sk, aug_msg)
}

#[cfg(test)]
use rand::{Rng, SeedableRng};

#[cfg(test)]
use rand::rngs::StdRng;

#[test]
#[cfg(feature = "py-bindings")]
fn test_generator() {
    assert_eq!(
        hex::encode(&Signature::generator().to_bytes()),
        "93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8"
    );
}

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

#[cfg(test)]
use hex::FromHex;

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
        &<[u8; 32]>::from_hex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb")
            .unwrap(),
    )
    .unwrap();

    let sig = sign(&sk, msg);
    assert!(verify(&sig, &sk.public_key(), msg));

    assert_eq!(sig.to_bytes(), <[u8; 96]>::from_hex("b45825c0ee7759945c0189b4c38b7e54231ebadc83a851bec3bb7cf954a124ae0cc8e8e5146558332ea152f63bf8846e04826185ef60e817f271f8d500126561319203f9acb95809ed20c193757233454be1562a5870570941a84605bd2c9c9a").unwrap());
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
    for idx in 0..4 {
        let derived = sk.derive_hardened(idx as u32);
        data.push((derived.public_key(), msg));
        let sig = sign(&derived, msg);
        agg1.aggregate(&sig);
        agg2 += &sig;
        sigs.push(sig);
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
}

#[test]
fn test_aggregate_duplicate_signature() {
    let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
    let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
    let msg = b"foobar";
    let mut agg = Signature::default();
    let mut data = Vec::<(PublicKey, &[u8])>::new();
    for _idx in 0..2 {
        data.push((sk.public_key(), msg));
        agg.aggregate(&sign(&sk, msg));
    }

    assert_eq!(agg.to_bytes(), <[u8; 96]>::from_hex("a1cca6540a4a06d096cb5b5fc76af5fd099476e70b623b8c6e4cf02ffde94fc0f75f4e17c67a9e350940893306798a3519368b02dc3464b7270ea4ca233cfa85a38da9e25c9314e81270b54d1e773a2ec5c3e14c62dac7abdebe52f4688310d3").unwrap());

    assert!(aggregate_verify(&agg, data));
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
}

#[test]
fn test_invalid_aggregate_signature() {
    let mut rng = StdRng::seed_from_u64(1337);
    let sk = [random_sk(&mut rng), random_sk(&mut rng)];
    let pk = [sk[0].public_key(), sk[1].public_key()];
    let msg: [&'static [u8]; 2] = [b"foo", b"foobar"];
    let sig = [sign(&sk[0], msg[0]), sign(&sk[1], msg[1])];
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
            (&pk1, &message1 as &[u8]),
            (&pk2, &message2),
            (&pk2, &message1),
            (&pk1, &message3),
            (&pk1, &message1),
            (&pk1, &message4)
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

    assert!(aggregate_verify(&aggsig, [] as [(&PublicKey, &[u8]); 0]));
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
    let sig1 = sign(&sk, &[0, 1, 2]);
    let sig2 = sign(&sk, &[0, 1, 2, 3]);

    assert!(hash(sig1) != hash(sig2));
    assert_eq!(hash(sign(&sk, &[0, 1, 2])), hash(sign(&sk, &[0, 1, 2])));
}

#[test]
fn test_debug() {
    let mut data = [0u8; 96];
    data[0] = 0xc0;
    let sig = Signature::from_bytes(&data).unwrap();
    assert_eq!(format!("{:?}", sig), hex::encode(data));
}

#[test]
fn test_to_from_clvm() {
    let mut a = Allocator::new();
    let bytes = hex::decode("b45825c0ee7759945c0189b4c38b7e54231ebadc83a851bec3bb7cf954a124ae0cc8e8e5146558332ea152f63bf8846e04826185ef60e817f271f8d500126561319203f9acb95809ed20c193757233454be1562a5870570941a84605bd2c9c9a").expect("hex::decode()");
    let ptr = a.new_atom(&bytes).expect("new_atom");

    let sig = Signature::from_clvm(&a, ptr).expect("from_clvm");
    assert_eq!(&sig.to_bytes()[..], &bytes[..]);

    let sig_ptr = sig.to_clvm(&mut a).expect("to_clvm");
    assert!(a.atom_eq(sig_ptr, ptr));
}

#[test]
fn test_from_clvm_failure() {
    let mut a = Allocator::new();
    let ptr = a.new_pair(a.one(), a.one()).expect("new_pair");
    assert_eq!(
        Signature::from_clvm(&a, ptr).unwrap_err(),
        clvm_traits::Error::ExpectedAtom(ptr)
    );
}

#[cfg(test)]
#[cfg(feature = "py-bindings")]
mod pytests {

    use super::*;
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
            let ret = Python::with_gil(|py| -> PyResult<()> {
                let string = sig.to_json_dict(py)?;
                let sig2 = Signature::from_json_dict(string.as_ref(py)).unwrap();
                assert_eq!(sig, sig2);
                Ok(())
            });
            assert!(ret.is_ok())
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
        let ret = Python::with_gil(|py| -> PyResult<()> {
            let err =
                Signature::from_json_dict(input.to_string().into_py(py).as_ref(py)).unwrap_err();
            assert_eq!(err.value(py).to_string(), msg.to_string());
            Ok(())
        });
        assert!(ret.is_ok())
    }
}
