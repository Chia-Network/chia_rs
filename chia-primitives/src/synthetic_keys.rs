use chia_bls::{PublicKey, SecretKey};
use num_bigint::BigInt;
use num_traits::Num;
use sha2::{digest::FixedOutput, Digest, Sha256};

pub trait SyntheticKeyExt {
    fn derive_synthetic(&self, hidden_puzzle_hash: &[u8; 32]) -> Self;
}

impl SyntheticKeyExt for PublicKey {
    fn derive_synthetic(&self, hidden_puzzle_hash: &[u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(self.to_bytes());
        hasher.update(hidden_puzzle_hash);
        let bytes: [u8; 32] = hasher.finalize_fixed().into();

        let value = BigInt::from_signed_bytes_be(&bytes);

        let group_order = BigInt::from_str_radix(
            "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001",
            16,
        )
        .unwrap();
        let modulo = ((&value % &group_order) + &group_order) % &group_order;
        let mut byte_vec = modulo.to_bytes_be().1;
        if byte_vec.len() < 32 {
            let pad = vec![0; 32 - byte_vec.len()];
            byte_vec.splice(0..0, pad);
        }

        let sk = SecretKey::from_bytes(&byte_vec.try_into().unwrap());
        let pk = sk.to_public_key();
        self.add(&pk)
    }
}

#[cfg(test)]
mod tests {
    use crate::puzzles::DEFAULT_HIDDEN_PUZZLE_HASH;

    use super::*;

    use chia_bls::{master_to_wallet_unhardened_intermediate, DerivableKey};
    use hex::ToHex;
    use hex_literal::hex;

    #[test]
    fn test_synthetic_keys() {
        let hex_keys = [
            "b0c8cf08fdbe7fdb7bb1795740153b944c32364b100c372a05833554cb97794563b096cb5f57bfa09f38d7aebb48704e",
            "8b1b92da63fdf8c4b53349da2fdd84685303587653f1a75826a56a97ea50b86ca8a0fbf6a5d6605c70b6be324bc59c85",
            "a472c01f0b32457aea348ef0493e1d394445df528e0d4139056ba6b4eb57eed593732c830acd897dab502f119d1ae2ff",
            "8b9e4040514e55110cd899b43a5fb8fa6f74e28620f80d20401101f88a77624128c818238073f618b72065a7a7264402",
            "ac334afc58318068c6ec2daffb336cedc8a01d382e87852c62846fa17f9249c8b0896d1c09a26c80ec945f93002d0ff4",
            "8d63ad4f29c7f163f6742f41bb3dc08ea6975ecad0b76324545e6154d89370a695b9ae803bc65c3384d8557f3de67a40",
            "b5d5540d7e5721688fa7876a49028135d42b67a0e73c257463f01775b1c973b6161973608469b3a42b20b0392aeca46c",
            "92fd0374247c22e2deaaccd844dc152b87a736d4df531fa94fdd04948295310c21a2fbe5ff6b25e12ae12afcc90716d8",
            "adda2cfe848768537074e91f4e08136fe85e7315e326063c6945314492e1eb6903911176dcbdb84637d49a26afbf5437",
            "b0d252b37fc5b50f281c1d27151963e13be1d6bc2f9f32e263806b03e843ff9198a6128247b9d51b64d28bc7c8646674",
            "95873a2fff6e139c257be5eee37262e0774920965c26483c9b32cceb565abbc74dcfb36679224fb7f7d5ac0060015aea",
            "8b8b469a973a5702bb0b51f774041da814c2b0d81a0d0a58b946c9c995be9dfaadc1501f0adf2088a66d67a4a6f92193",
            "b27b87ea6b1e9653b54d2377e95708444f886ca0fc1728889bf3afee2f8cbe4c618b7127e9f38a189e6d56dd7933cfff",
            "b46d152384d888737aebe52bb9127314f678733c45948b00075575db79b732a2bbfa47dab0886863ade7f5fbdc4a14fa",
            "ada6da1ce6464d22dcbc1fe4396a0d1aa8a486fc7094f89a5d11a81cf75a1209eca7bae3b1d943dcff6e39c163d29fb5",
            "b3b4ceea11bbc6fafb5800caa593385644a3262245357e5013be5c1cf622bf7cb0b667e586269c346459c3b5faf0eaef"
        ];

        let sk = SecretKey::from_bytes(&hex!(
            "6bb19282e27bc6e7e397fb19efc2627a412410fdfd13bf14f4ce5bfdce084c71"
        ));
        let pk = sk.to_public_key();
        let intermediate = master_to_wallet_unhardened_intermediate(&pk);

        for (index, hex) in hex_keys.iter().enumerate() {
            let key = intermediate
                .derive_unhardened(index as u32)
                .derive_synthetic(&DEFAULT_HIDDEN_PUZZLE_HASH);
            assert_eq!(key.to_bytes().encode_hex::<String>(), *hex);
        }
    }
}
