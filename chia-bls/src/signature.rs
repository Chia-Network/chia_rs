use crate::public_key::PublicKey;
use crate::secret_key::SecretKey;
use bls12_381_plus::{
    multi_miller_loop, ExpandMsgXmd, G1Affine, G2Affine, G2Prepared, G2Projective,
};
use group::{Curve, Group};
use std::borrow::Borrow;
use std::convert::AsRef;
use std::ops::Neg;

#[derive(PartialEq, Eq, Debug)]
pub struct Signature(pub(crate) G2Projective);

impl Signature {
    pub fn from_bytes(buf: &[u8; 96]) -> Option<Signature> {
        G2Affine::from_compressed(buf)
            .map(|p| Self(G2Projective::from(&p)))
            .into()
    }

    pub fn to_bytes(&self) -> [u8; 96] {
        self.0.to_affine().to_compressed()
    }

    pub fn aggregate(&mut self, sig: &Signature) {
        self.0 += sig.0;
    }

    pub fn is_valid(&self) -> bool {
        self.0.is_on_curve().unwrap_u8() == 1
    }
}

impl Default for Signature {
    fn default() -> Self {
        Signature(G2Projective::identity())
    }
}

fn hash_msg<Msg: AsRef<[u8]>>(pk: &PublicKey, msg: Msg) -> G2Projective {
    let mut prepended_msg = pk.to_bytes().to_vec();
    prepended_msg.extend_from_slice(msg.as_ref());
    // domain separation tag
    const CIPHER_SUITE: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";
    G2Projective::hash::<ExpandMsgXmd<sha2::Sha256>>(&prepended_msg, CIPHER_SUITE)
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
    if !key.is_valid() || !sig.is_valid() {
        return false;
    }
    let a = hash_msg(key, msg);
    let g1 = G1Affine::generator().neg();

    multi_miller_loop(&[
        (&key.0.to_affine(), &G2Prepared::from(a.to_affine())),
        (&g1, &G2Prepared::from(sig.0.to_affine())),
    ])
    .final_exponentiation()
    .is_identity()
    .into()
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
    let mut store = Vec::<(G1Affine, G2Prepared)>::new();

    for (key, msg) in data.into_iter() {
        let key = key.borrow();
        if !key.is_valid() {
            return false;
        }
        store.push((
            key.0.to_affine(),
            G2Prepared::from(hash_msg(key, msg.borrow()).to_affine()),
        ));
    }

    if store.is_empty() {
        // if we have exactly zero messages to verify, the only correct
        // signature is the identity
        // This is an optimization for the edge case of having 0 messages
        return sig == &Signature::default();
    }

    store.push((
        G1Affine::generator().neg(),
        G2Prepared::from(sig.0.to_affine()),
    ));

    let mut terms = Vec::<(&G1Affine, &G2Prepared)>::new();
    for (g1, g2) in &store {
        terms.push((g1, g2));
    }

    // multi_miller_loop takes a slice of *references*, which means we need to build
    // both a vector owning the elements (G1Affine and G2Prepared) in addition to a
    // vector holding references into it.
    multi_miller_loop(terms.as_slice())
        .final_exponentiation()
        .is_identity()
        .into()
}

pub fn sign<Msg: AsRef<[u8]>>(sk: &SecretKey, msg: Msg) -> Signature {
    let g2 = hash_msg(&sk.public_key(), msg);
    Signature(g2 * sk.0)
}

#[cfg(test)]
use rand::{Rng, SeedableRng};

#[cfg(test)]
use rand::rngs::StdRng;

#[test]
fn test_from_bytes() {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 96];
    for _i in 0..50 {
        rng.fill(data.as_mut_slice());
        // just any random bytes are not a valid signature and should fail
        assert_eq!(Signature::from_bytes(&data), None);
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
        let sig = sign(&sk, &msg);
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

    let sig = sign(&sk, &msg);
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
    let mut agg = Signature::default();
    let mut data = Vec::<(PublicKey, &[u8])>::new();
    for idx in 0..4 {
        let derived = sk.derive_hardened(idx as u32);
        data.push((derived.public_key(), msg));
        agg.aggregate(&sign(&derived, msg));
    }
    assert_eq!(agg.to_bytes(), <[u8; 96]>::from_hex("87bce2c588f4257e2792d929834548c7d3af679272cb4f8e1d24cf4bf584dd287aa1d9f5e53a86f288190db45e1d100d0a5e936079a66a709b5f35394cf7d52f49dd963284cb5241055d54f8cf48f61bc1037d21cae6c025a7ea5e9f4d289a18").unwrap());

    // ensure the aggregate signature verifies OK
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

    assert!(aggregate_verify(&agg, [(&pk[0], msg[0])]) == false);
    assert!(aggregate_verify(&agg, [(&pk[1], msg[1])]) == false);
    // public keys mixed with the wrong message
    assert!(aggregate_verify(&agg, [(&pk[0], msg[1]), (&pk[1], msg[0])]) == false);
    assert!(aggregate_verify(&agg, [(&pk[1], msg[0]), (&pk[0], msg[1])]) == false);
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

    let sig1 = sign(&sk1, &message1);
    let sig2 = sign(&sk2, &message2);
    let sig3 = sign(&sk2, &message1);
    let sig4 = sign(&sk1, &message3);
    let sig5 = sign(&sk1, &message1);
    let sig6 = sign(&sk1, &message4);

    let agg_sig_l = aggregate(&[sig1, sig2]);
    let agg_sig_r = aggregate(&[sig3, sig4, sig5]);
    let aggsig = aggregate(&[agg_sig_l, agg_sig_r, sig6]);

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
    assert_eq!(sign(&sk, &[1_u8, 2, 3]), Signature::default());
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
    let mut agg_sig = sign(&sk1, &message1);
    let mut pairs: Vec<(PublicKey, &[u8])> = vec![(pk1, &message1)];

    for _i in 0..10 {
        let sk = random_sk(&mut rng);
        let pk = sk.public_key();
        pairs.push((pk, &message1));
        let sig = sign(&sk, &message1);
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

    let sig1 = sign(&sk1, &message1);
    let sig2 = sign(&sk2, &message2);
    let sig3 = sign(&sk2, &message3);
    let sig4 = sign(&sk1, &message4);

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
    let sig1 = sign(&sk1, &msg1);
    let sig1v = sig1.to_bytes();

    assert!(verify(&sig1, &pk1, &msg1));
    assert!(verify(
        &Signature::from_bytes(&sig1v).unwrap(),
        &PublicKey::from_bytes(&pk1v).unwrap(),
        &msg1
    ));

    let sk2 = SecretKey::from_seed(&[5_u8; 32]);
    let pk2 = sk2.public_key();
    let pk2v = pk2.to_bytes();
    let sig2 = sign(&sk2, &msg2);
    let sig2v = sig2.to_bytes();

    assert!(verify(&sig2, &pk2, &msg2));
    assert!(verify(
        &Signature::from_bytes(&sig2v).unwrap(),
        &PublicKey::from_bytes(&pk2v).unwrap(),
        &msg2
    ));

    // Wrong G2Element
    assert!(!verify(&sig2, &pk1, &msg1));
    assert!(!verify(
        &Signature::from_bytes(&sig2v).unwrap(),
        &PublicKey::from_bytes(&pk1v).unwrap(),
        &msg1
    ));
    // Wrong msg
    assert!(!verify(&sig1, &pk1, &msg2));
    assert!(!verify(
        &Signature::from_bytes(&sig1v).unwrap(),
        &PublicKey::from_bytes(&pk1v).unwrap(),
        &msg2
    ));
    // Wrong pk
    assert!(!verify(&sig1, &pk2, &msg1));
    assert!(!verify(
        &Signature::from_bytes(&sig1v).unwrap(),
        &PublicKey::from_bytes(&pk2v).unwrap(),
        &msg1
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
