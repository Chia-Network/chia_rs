mod secp256k1;
mod secp256r1;

pub use secp256k1::*;
pub use secp256r1::*;

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    use super::*;

    #[test]
    fn test_secp256k1_key() -> anyhow::Result<()> {
        let mut rng = ChaCha8Rng::seed_from_u64(1337);

        let sk = K1SecretKey::from_bytes(&rng.gen())?;
        assert_eq!(
            hex::encode(sk.to_bytes()),
            "ae491886341a539a1ccfaffcc9c78650ad1adc6270620c882b8d29bf6b9bc4cd"
        );
        assert_eq!(format!("{sk:?}"), "K1SecretKey(...)");

        let pk = sk.public_key();
        assert_eq!(
            hex::encode(pk.to_bytes()),
            "02827cdbbed87e45683d448be2ea15fb72ba3732247bda18474868cf5456123fb4"
        );
        assert_eq!(
            format!("{pk:?}"),
            "K1PublicKey(02827cdbbed87e45683d448be2ea15fb72ba3732247bda18474868cf5456123fb4)"
        );
        assert_eq!(
            format!("{pk}"),
            "02827cdbbed87e45683d448be2ea15fb72ba3732247bda18474868cf5456123fb4"
        );

        let message_hash: [u8; 32] = rng.gen();
        let sig = sk.sign_prehashed(&message_hash)?;
        assert_eq!(
            hex::encode(sig.to_bytes()),
            "6f07897d1d28b8698af5dec5ca06907b1304b227dc9f740b8c4065cf04d5e8653ae66aa17063e7120ee7f22fae54373b35230e259244b90400b65cf00d86c591"
        );
        assert_eq!(
            format!("{sig:?}"),
            "K1Signature(6f07897d1d28b8698af5dec5ca06907b1304b227dc9f740b8c4065cf04d5e8653ae66aa17063e7120ee7f22fae54373b35230e259244b90400b65cf00d86c591)"
        );
        assert_eq!(
            format!("{sig}"),
            "6f07897d1d28b8698af5dec5ca06907b1304b227dc9f740b8c4065cf04d5e8653ae66aa17063e7120ee7f22fae54373b35230e259244b90400b65cf00d86c591"
        );

        assert!(pk.verify_prehashed(&message_hash, &sig));

        Ok(())
    }

    #[test]
    fn test_secp256r1_key() -> anyhow::Result<()> {
        let mut rng = ChaCha8Rng::seed_from_u64(1337);

        let sk = R1SecretKey::from_bytes(&rng.gen())?;
        assert_eq!(
            hex::encode(sk.to_bytes()),
            "ae491886341a539a1ccfaffcc9c78650ad1adc6270620c882b8d29bf6b9bc4cd"
        );
        assert_eq!(format!("{sk:?}"), "R1SecretKey(...)");

        let pk = sk.public_key();
        assert_eq!(
            hex::encode(pk.to_bytes()),
            "037dc85102f5eb7867b9580fea8b242c774173e1a47db320c798242d3a7a7579e4"
        );
        assert_eq!(
            format!("{pk:?}"),
            "R1PublicKey(037dc85102f5eb7867b9580fea8b242c774173e1a47db320c798242d3a7a7579e4)"
        );
        assert_eq!(
            format!("{pk}"),
            "037dc85102f5eb7867b9580fea8b242c774173e1a47db320c798242d3a7a7579e4"
        );

        let message_hash: [u8; 32] = rng.gen();
        let sig = sk.sign_prehashed(&message_hash)?;
        assert_eq!(
            hex::encode(sig.to_bytes()),
            "550e83da8cf9b2d407ed093ae213869ebd7ceaea603920f87d535690e52b40537915d8fe3d5a96c87e700c56dc638c32f7a2954f2ba409367d1a132000cc2228"
        );
        assert_eq!(
            format!("{sig:?}"),
            "R1Signature(550e83da8cf9b2d407ed093ae213869ebd7ceaea603920f87d535690e52b40537915d8fe3d5a96c87e700c56dc638c32f7a2954f2ba409367d1a132000cc2228)"
        );
        assert_eq!(
            format!("{sig}"),
            "550e83da8cf9b2d407ed093ae213869ebd7ceaea603920f87d535690e52b40537915d8fe3d5a96c87e700c56dc638c32f7a2954f2ba409367d1a132000cc2228"
        );

        assert!(pk.verify_prehashed(&message_hash, &sig));

        Ok(())
    }
}
