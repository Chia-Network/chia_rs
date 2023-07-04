use blst::{
    blst_p2 as P2, blst_p2_add_affine, blst_p2_affine as P2Affine, blst_p2_affine_compress,
    blst_p2_affine_generator, blst_p2_affine_in_g2, blst_p2_affine_is_inf, blst_p2_from_affine,
    blst_p2_to_affine, blst_p2_uncompress, BLST_ERROR,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub(crate) P2Affine);

impl Signature {
    pub fn validate(bytes: &[u8; 96]) -> Option<Self> {
        let result = Self::from_bytes(bytes)?;

        if result.is_valid(true) {
            Some(result)
        } else {
            None
        }
    }

    pub fn from_bytes(bytes: &[u8; 96]) -> Option<Self> {
        if (bytes[0] & 0x80) == 0 {
            let mut p2_affine = P2Affine::default();
            if unsafe { blst_p2_uncompress(&mut p2_affine, bytes.as_ptr()) }
                == BLST_ERROR::BLST_SUCCESS
            {
                Some(Self(p2_affine))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        unsafe {
            blst_p2_affine_compress(bytes.as_mut_ptr(), &self.0);
        }
        bytes
    }

    pub fn is_valid(&self, check_infinity: bool) -> bool {
        unsafe {
            (!check_infinity || !blst_p2_affine_is_inf(&self.0)) && blst_p2_affine_in_g2(&self.0)
        }
    }

    pub fn add(&self, public_key: &Self) -> Self {
        let mut p2 = P2::default();
        unsafe {
            blst_p2_from_affine(&mut p2, &self.0);
        }

        let mut output_p2 = P2::default();
        unsafe {
            blst_p2_add_affine(&mut output_p2, &p2, &public_key.0);
        }

        let mut p2_affine = P2Affine::default();
        unsafe {
            blst_p2_to_affine(&mut p2_affine, &output_p2);
        }
        Self(p2_affine)
    }
}

impl Default for Signature {
    fn default() -> Self {
        Self(unsafe { *blst_p2_affine_generator() })
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::{PublicKey, SecretKey};

//     use super::*;

//     use hex::FromHex;
//     use rand::rngs::StdRng;
//     use rand::{Rng, SeedableRng};

//     #[test]
//     fn test_from_bytes() {
//         let mut rng = StdRng::seed_from_u64(1337);
//         let mut data = [0u8; 96];
//         for _i in 0..50 {
//             rng.fill(data.as_mut_slice());
//             // just any random bytes are not a valid signature and should fail
//             assert_eq!(Signature::from_bytes(&data), None);
//         }
//     }

//     #[test]
//     fn test_roundtrip() {
//         let mut rng = StdRng::seed_from_u64(1337);
//         let mut data = [0u8; 32];
//         let mut msg = [0u8; 32];
//         rng.fill(msg.as_mut_slice());
//         for _i in 0..50 {
//             rng.fill(data.as_mut_slice());
//             let sk = SecretKey::from_seed(&data);
//             let sig = sk.sign(&msg);
//             let bytes = sig.to_bytes();
//             let sig2 = Signature::from_bytes(&bytes).unwrap();
//             assert_eq!(sig, sig2);
//         }
//     }

//     #[test]
//     fn test_random_verify() {
//         let mut rng = StdRng::seed_from_u64(1337);
//         let mut data = [0u8; 32];
//         let mut msg = [0u8; 32];
//         rng.fill(msg.as_mut_slice());
//         for _i in 0..20 {
//             rng.fill(data.as_mut_slice());
//             let sk = SecretKey::from_seed(&data);
//             let pk = sk.to_public_key();
//             let sig = sk.sign(&msg);
//             assert!(pk.verify(&msg, &sig));

//             let bytes = sig.to_bytes();
//             let sig2 = Signature::from_bytes(&bytes).unwrap();
//             assert!(pk.verify(&msg, &sig2));
//         }
//     }

//     #[test]
//     fn test_verify() {
//         // test case from:
//         // from blspy import PrivateKey
//         // from blspy import AugSchemeMPL
//         // sk = PrivateKey.from_bytes(bytes.fromhex("52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb"))
//         // data = b"foobar"
//         // print(AugSchemeMPL.sign(sk, data))
//         let msg = b"foobar";
//         let sk = SecretKey::from_bytes(
//             &<[u8; 32]>::from_hex(
//                 "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb",
//             )
//             .unwrap(),
//         );

//         let sig = sk.sign(msg);
//         let pk = sk.to_public_key();
//         assert!(pk.verify(msg, &sig));

//         assert_eq!(sig.to_bytes(), <[u8; 96]>::from_hex("b45825c0ee7759945c0189b4c38b7e54231ebadc83a851bec3bb7cf954a124ae0cc8e8e5146558332ea152f63bf8846e04826185ef60e817f271f8d500126561319203f9acb95809ed20c193757233454be1562a5870570941a84605bd2c9c9a").unwrap());
//     }

//     #[test]
//     fn test_aggregate_signature() {
//         let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
//         let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap());
//         let msg = b"foobar";
//         let mut agg = Signature::default();
//         let mut public_keys = Vec::new();
//         let mut messages = Vec::new();

//         for idx in 0..4 {
//             let derived = sk.derive_hardened(idx as u32);
//             public_keys.push(&derived.to_public_key());
//             messages.push(msg.as_slice());
//             agg = agg.add(&derived.sign(msg));
//         }

//         assert_eq!(agg.to_bytes(), <[u8; 96]>::from_hex("87bce2c588f4257e2792d929834548c7d3af679272cb4f8e1d24cf4bf584dd287aa1d9f5e53a86f288190db45e1d100d0a5e936079a66a709b5f35394cf7d52f49dd963284cb5241055d54f8cf48f61bc1037d21cae6c025a7ea5e9f4d289a18").unwrap());

//         // ensure the aggregate signature verifies OK
//         assert!(agg.aggregate_verify(&public_keys, &messages, true));
//     }

//     fn random_sk<R: Rng>(rng: &mut R) -> SecretKey {
//         let mut data = [0u8; 64];
//         rng.fill(data.as_mut_slice());
//         SecretKey::from_seed(&data)
//     }

//     #[test]
//     fn test_aggregate_signature_separate_msg() {
//         let mut rng = StdRng::seed_from_u64(1337);
//         let sk = [random_sk(&mut rng), random_sk(&mut rng)];
//         let mut pk = [&sk[0].to_public_key(), &sk[1].to_public_key()];
//         let mut msg: [&[u8]; 2] = [b"foo", b"foobar"];
//         let sig = [sk[0].sign(msg[0]), sk[1].sign(msg[1])];
//         let agg = sig[0].add(&sig[1]);

//         assert!(agg.aggregate_verify(&pk, &msg, true));
//         // order does not matter
//         pk.reverse();
//         msg.reverse();
//         assert!(agg.aggregate_verify(&pk, &msg, true));
//     }

//     #[test]
//     fn test_aggregate_signature_identity() {
//         // when verifying 0 messages, an identity signature is considered valid
//         assert!(Signature::default().aggregate_verify(&[], &[], true));
//     }

//     #[test]
//     fn test_invalid_aggregate_signature() {
//         let mut rng = StdRng::seed_from_u64(1337);
//         let sk = [random_sk(&mut rng), random_sk(&mut rng)];
//         let pk = [sk[0].to_public_key(), sk[1].to_public_key()];
//         let msg: [&[u8]; 2] = [b"foo", b"foobar"];
//         let sig = [sk[0].sign(msg[0]), sk[1].sign(msg[1])];
//         let agg = sig[0].add(&sig[1]);

//         assert!(!agg.aggregate_verify(&[&pk[0]], &[&msg[0]], true));
//         assert!(!agg.aggregate_verify(&[&pk[1]], &[&msg[1]], true));
//         // public keys mixed with the wrong message
//         assert!(!agg.aggregate_verify(&[&pk[0], &pk[1]], &[&msg[1], &msg[0]], true));
//         assert!(!agg.aggregate_verify(&[&pk[1], &pk[0]], &[&msg[0], &msg[1]], true));
//     }

//     #[test]
//     fn test_vector_2_aggregate_of_aggregates() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Chia test vector 2 (Augmented, aggregate of aggregates)"
//         let message1 = [1_u8, 2, 3, 40];
//         let message2 = [5_u8, 6, 70, 201];
//         let message3 = [9_u8, 10, 11, 12, 13];
//         let message4 = [15_u8, 63, 244, 92, 0, 1];

//         let sk1 = SecretKey::from_seed(&[2_u8; 32]);
//         let sk2 = SecretKey::from_seed(&[3_u8; 32]);

//         let pk1 = sk1.to_public_key();
//         let pk2 = sk2.to_public_key();

//         let sig1 = sk1.sign(&message1);
//         let sig2 = sk2.sign(&message2);
//         let sig3 = sk2.sign(&message1);
//         let sig4 = sk1.sign(&message3);
//         let sig5 = sk1.sign(&message1);
//         let sig6 = sk1.sign(&message4);

//         let agg_sig_l = sig1.add(&sig2);
//         let agg_sig_r = sig3.add(&sig4).add(&sig5);
//         let aggsig = agg_sig_l.add(&agg_sig_r).add(&sig6);

//         assert!(aggsig.aggregate_verify(
//             &[&pk1, &pk2, &pk2, &pk1, &pk1, &pk1],
//             &[&message1, &message2, &message1, &message3, &message1, &message4],
//             true
//         ));

//         assert_eq!(
//             aggsig.to_bytes(),
//             <[u8; 96]>::from_hex(
//                 "a1d5360dcb418d33b29b90b912b4accde535cf0e52caf467a005dc632d9f7af44b6c4e9acd4\
//             6eac218b28cdb07a3e3bc087df1cd1e3213aa4e11322a3ff3847bbba0b2fd19ddc25ca964871\
//             997b9bceeab37a4c2565876da19382ea32a962200"
//             )
//             .unwrap()
//         );
//     }

//     #[test]
//     fn test_signature_zero_key() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Should sign with the zero key"
//         let sk = SecretKey::from_bytes(&[0; 32]);
//         assert_eq!(sk.sign(&[1_u8, 2, 3]), Signature::default());
//     }

//     #[test]
//     fn test_aggregate_many_g2_elements_diff_message() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Should Aug aggregate many G2Elements, diff message"

//         let mut rng = StdRng::seed_from_u64(1337);

//         let mut public_keys = Vec::new();
//         let mut messages = Vec::new();
//         let mut sigs = Vec::<Signature>::new();

//         for i in 0..80 {
//             let message = vec![0_u8, 100, 2, 45, 64, 12, 12, 63, i];
//             let sk = random_sk(&mut rng);
//             let sig = sk.sign(&message);
//             public_keys.push(&sk.to_public_key());
//             messages.push(message.as_slice());
//             sigs.push(sig);
//         }

//         let aggsig = sigs.into_iter().reduce(|a, b| a.add(&b)).unwrap();

//         assert!(aggsig.aggregate_verify(&public_keys, &messages, true));
//     }

//     #[test]
//     fn test_aggregate_identity() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Aggregate Verification of zero items with infinity should pass"
//         let sig = Signature::default();
//         assert_eq!(sig, Signature::default());
//         assert!(sig.aggregate_verify(&[], &[], true));
//     }

//     #[test]
//     fn test_aggregate_multiple_levels_degenerate() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Should aggregate with multiple levels, degenerate"

//         let mut rng = StdRng::seed_from_u64(1337);

//         let message1 = [100_u8, 2, 254, 88, 90, 45, 23];
//         let sk1 = random_sk(&mut rng);
//         let pk1 = sk1.to_public_key();
//         let mut agg_sig = sk1.sign(&message1);

//         let mut public_keys = vec![pk1];
//         let mut messages = vec![message1.as_slice()];

//         for _i in 0..10 {
//             let sk = random_sk(&mut rng);
//             let pk = sk.to_public_key();
//             public_keys.push(pk);
//             messages.push(&message1);
//             let sig = sk.sign(&message1);
//             agg_sig = agg_sig.add(&sig);
//         }

//         assert!(agg_sig.aggregate_verify(public_keys.as_ref(), &messages, true));
//     }

//     #[test]
//     fn test_aggregate_multiple_levels_different_messages() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Should aggregate with multiple levels, different messages"

//         let mut rng = StdRng::seed_from_u64(1337);

//         let message1 = [100_u8, 2, 254, 88, 90, 45, 23];
//         let message2 = [192_u8, 29, 2, 0, 0, 45, 23];
//         let message3 = [52_u8, 29, 2, 0, 0, 45, 102];
//         let message4 = [99_u8, 29, 2, 0, 0, 45, 222];

//         let sk1 = random_sk(&mut rng);
//         let sk2 = random_sk(&mut rng);

//         let pk1 = sk1.to_public_key();
//         let pk2 = sk2.to_public_key();

//         let sig1 = sk1.sign(&message1);
//         let sig2 = sk2.sign(&message2);
//         let sig3 = sk2.sign(&message3);
//         let sig4 = sk1.sign(&message4);

//         let agg_sig_l = sig1.add(&sig2);
//         let agg_sig_r = sig3.add(&sig4);
//         let agg_sig = agg_sig_l.add(&agg_sig_r);

//         assert!(agg_sig.aggregate_verify(
//             &[&pk1, &pk2, &pk2, &pk1],
//             &[&message1, &message2, &message3, &message4],
//             true
//         ));
//     }

//     #[test]
//     fn test_aug_scheme() {
//         // test case from: bls-signatures/src/test.cpp
//         // "Aug Scheme"

//         let msg1 = [7_u8, 8, 9];
//         let msg2 = [10_u8, 11, 12];

//         let sk1 = SecretKey::from_seed(&[4_u8; 32]);
//         let pk1 = sk1.to_public_key();
//         let pk1v = pk1.to_bytes();
//         let sig1 = sk1.sign(&msg1);
//         let sig1v = sig1.to_bytes();

//         assert!(pk1.verify(&msg1, &sig1));
//         assert!(PublicKey::from_bytes(&pk1v)
//             .unwrap()
//             .verify(&msg1, &Signature::from_bytes(&sig1v).unwrap(),));

//         let sk2 = SecretKey::from_seed(&[5_u8; 32]);
//         let pk2 = sk2.to_public_key();
//         let pk2v = pk2.to_bytes();
//         let sig2 = sk2.sign(&msg2);
//         let sig2v = sig2.to_bytes();

//         assert!(pk2.verify(&msg2, &sig2));
//         assert!(PublicKey::from_bytes(&pk2v)
//             .unwrap()
//             .verify(&msg2, &Signature::from_bytes(&sig2v).unwrap(),));

//         // Wrong G2Element
//         assert!(!pk1.verify(&msg1, &sig2));
//         assert!(!PublicKey::from_bytes(&pk1v)
//             .unwrap()
//             .verify(&msg1, &Signature::from_bytes(&sig2v).unwrap(),));
//         // Wrong msg
//         assert!(!pk1.verify(&msg2, &sig1));
//         assert!(!PublicKey::from_bytes(&pk1v)
//             .unwrap()
//             .verify(&msg2, &Signature::from_bytes(&sig1v).unwrap(),));
//         // Wrong pk
//         assert!(!pk2.verify(&msg1, &sig1));
//         assert!(!PublicKey::from_bytes(&pk2v)
//             .unwrap()
//             .verify(&msg1, &Signature::from_bytes(&sig1v).unwrap(),));

//         let aggsig = sig1.add(&sig2);
//         let aggsigv = aggsig.to_bytes();
//         assert!(aggsig.aggregate_verify(&[&pk1, &pk2], &[&msg1, &msg2], true));
//         assert!(&Signature::from_bytes(&aggsigv).unwrap().aggregate_verify(
//             &[&pk1, &pk2],
//             &[&msg1, &msg2],
//             true
//         ));
//     }
// }
