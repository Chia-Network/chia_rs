use crate::{DerivableKey, PublicKey};
use blst::*;
use chia_traits::chia_error::{Error, Result};
use chia_traits::{read_bytes, Streamable};
use hkdf::HkdfExtract;
use sha2::{Digest, Sha256};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem::MaybeUninit;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct SecretKey(pub(crate) blst_scalar);

fn flip_bits(input: [u8; 32]) -> [u8; 32] {
    let mut ret = [0; 32];
    for i in 0..32 {
        ret[i] = input[i] ^ 0xff;
    }
    ret
}

fn ikm_to_lamport_sk(ikm: &[u8; 32], salt: &[u8; 4]) -> [u8; 255 * 32] {
    let mut extracter = HkdfExtract::<Sha256>::new(Some(salt));
    extracter.input_ikm(ikm);
    let (_, h) = extracter.finalize();

    let mut output = [0_u8; 255 * 32];
    h.expand(&[], &mut output).unwrap();
    output
}

fn to_lamport_pk(ikm: [u8; 32], idx: u32) -> [u8; 32] {
    let not_ikm = flip_bits(ikm);
    let salt = idx.to_be_bytes();

    let mut lamport0 = ikm_to_lamport_sk(&ikm, &salt);
    let mut lamport1 = ikm_to_lamport_sk(&not_ikm, &salt);

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
    hasher.finalize().try_into().unwrap()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().try_into().unwrap()
}

pub fn is_all_zero(buf: &[u8]) -> bool {
    let (prefix, aligned, suffix) = unsafe { buf.align_to::<u128>() };

    prefix.iter().all(|&x| x == 0)
        && suffix.iter().all(|&x| x == 0)
        && aligned.iter().all(|&x| x == 0)
}

impl SecretKey {
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
            blst_bendian_from_scalar(bytes.as_mut_ptr() as *mut u8, &scalar.assume_init());
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
            return Err(Error::Custom(
                "SecretKey byte data must be less than the group order".to_string(),
            ));
        }

        Ok(Self(pk))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        unsafe {
            let mut bytes = MaybeUninit::<[u8; 32]>::uninit();
            blst_bendian_from_scalar(bytes.as_mut_ptr() as *mut u8, &self.0);
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

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(&self.to_bytes());
        Ok(())
    }

    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Self::from_bytes(read_bytes(input, 32)?.try_into().unwrap())
    }
}

impl Hash for SecretKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.to_bytes())
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

#[cfg(test)]
use hex::FromHex;

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

    for idx in 0..4_usize {
        let derived = sk.derive_unhardened(idx as u32);
        assert_eq!(
            derived.to_bytes(),
            <[u8; 32]>::from_hex(derived_hex[idx]).unwrap()
        )
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

    for idx in 0..derived_hex.len() {
        let derived = sk.derive_hardened(idx as u32);
        assert_eq!(
            derived.to_bytes(),
            <[u8; 32]>::from_hex(derived_hex[idx]).unwrap()
        )
    }
}

#[cfg(test)]
use rand::{Rng, SeedableRng};

#[cfg(test)]
use rand::rngs::StdRng;

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
            Error::Custom("SecretKey byte data must be less than the group order".to_string())
        );
    }
}

#[test]
fn test_from_bytes_zero() {
    let data = [0u8; 32];
    let _sk = SecretKey::from_bytes(&data).unwrap();
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
