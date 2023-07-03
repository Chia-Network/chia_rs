use blst::{
    blst_bendian_from_scalar, blst_keygen_v3, blst_p1_affine as P1Affine, blst_p2 as P2,
    blst_p2_affine as P2Affine, blst_scalar as Scalar, blst_scalar_from_be_bytes,
    blst_sk_add_n_check, blst_sk_check, blst_sk_to_pk2_in_g1,
};
use blst::{blst_p2_to_affine, blst_sign_pk_in_g1};
use sha2::digest::FixedOutput;
use sha2::{Digest, Sha256};

use crate::aug_scheme::{hash_to_g2, prepend_message};
use crate::DerivableKey;
use crate::PublicKey;
use crate::Signature;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretKey(pub(crate) Scalar);

impl SecretKey {
    pub fn from_seed(seed: &[u8]) -> Self {
        assert!(seed.len() >= 32);
        let info = [];
        let mut scalar = Scalar::default();
        unsafe {
            blst_keygen_v3(
                &mut scalar,
                seed.as_ptr(),
                seed.len(),
                info.as_ptr(),
                info.len(),
            );
        }
        Self(scalar)
    }

    pub fn from_bytes(b: &[u8; 32]) -> Option<Self> {
        let mut scalar = Scalar::default();
        unsafe {
            let result = blst_scalar_from_be_bytes(&mut scalar, b.as_ptr(), b.len());

            if result && blst_sk_check(&scalar) {
                Some(Self(scalar))
            } else {
                None
            }
        }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        unsafe {
            blst_bendian_from_scalar(bytes.as_mut_ptr(), &self.0);
        }
        bytes
    }

    pub fn to_public_key(&self) -> PublicKey {
        let mut p1_affine = P1Affine::default();
        unsafe {
            blst_sk_to_pk2_in_g1(std::ptr::null_mut(), &mut p1_affine, &self.0);
        }
        PublicKey(p1_affine)
    }

    pub fn add(&self, secret_key: &Self) -> Self {
        let mut scalar = Scalar::default();
        unsafe {
            assert!(blst_sk_add_n_check(&mut scalar, &self.0, &secret_key.0));
        }
        Self(scalar)
    }

    pub fn derive_hardened(&self, _index: u32) -> Self {
        // Blocked by https://github.com/supranational/blst/issues/173
        todo!()
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        let message = prepend_message(&self.to_public_key(), message);
        let p2 = hash_to_g2(&message);

        let mut signature_p2 = P2::default();
        unsafe {
            blst_sign_pk_in_g1(&mut signature_p2, &p2, &self.0);
        }

        let mut signature_p2_affine = P2Affine::default();
        unsafe {
            blst_p2_to_affine(&mut signature_p2_affine, &signature_p2);
        }

        Signature(signature_p2_affine)
    }
}

impl DerivableKey for SecretKey {
    fn derive_unhardened(&self, index: u32) -> Self {
        let pk = self.to_public_key();

        let mut hasher = Sha256::new();
        hasher.update(pk.to_bytes());
        hasher.update(index.to_be_bytes());
        let digest: [u8; 32] = hasher.finalize_fixed().into();

        let new_sk = Self::from_bytes(&digest).unwrap();
        self.add(&new_sk)
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
        // (Seed, SecretKey)
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
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let derived_hex = [
            "399638f99d446500f3c3a363f24c2b0634ad7caf646f503455093f35f29290bd",
            "3dcb4098ad925d8940e2f516d2d5a4dbab393db928a8c6cb06b93066a09a843a",
            "13115c8fb68a3d667938dac2ffc6b867a4a0f216bbb228aa43d6bdde14245575",
            "52e7e9f2fb51f2c5705aea8e11ac82737b95e664ae578f015af22031d956f92b",
        ];

        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        for (index, hex) in derived_hex.iter().enumerate() {
            let derived = sk.derive_unhardened(index as u32);
            assert_eq!(derived.to_bytes(), <[u8; 32]>::from_hex(hex).unwrap())
        }
    }

    #[test]
    fn test_public_key() {
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
            let pk = sk.to_public_key();
            assert_eq!(
                pk,
                PublicKey::from_bytes(&<[u8; 48]>::from_hex(pk_hex).unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn test_derive_hardened() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let derived_hex = [
            "05eccb2d70e814f51a30d8b9965505605c677afa97228fa2419db583a8121db9",
            "612ae96bdce2e9bc01693ac579918fbb559e04ec365cce9b66bb80e328f62c46",
            "5df14a0a34fd6c30a80136d4103f0a93422ce82d5c537bebbecbc56e19fee5b9",
            "3ea55db88d9a6bf5f1d9c9de072e3c9a56b13f4156d72fca7880cd39b4bd4fdc",
        ];

        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        for (index, hex) in derived_hex.iter().enumerate() {
            let derived = sk.derive_hardened(index as u32);
            assert_eq!(derived.to_bytes(), <[u8; 32]>::from_hex(hex).unwrap())
        }
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
            assert_eq!(SecretKey::from_bytes(&data), None);
        }
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
            assert_eq!(sk.to_public_key(), sk2.to_public_key());
        }
    }
}
