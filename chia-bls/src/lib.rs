mod aug_scheme;
mod derivable_key;
mod derive_keys;
mod public_key;
mod secret_key;
mod signature;

pub use derivable_key::*;
pub use derive_keys::*;
pub use public_key::*;
pub use secret_key::*;
pub use signature::*;

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    use bip39::Mnemonic;
    use hex::ToHex;
    use sha2::{Digest, Sha256};

    const MNEMONIC: &str = "crunch fox track good false smooth token lottery match surprise fiber awesome shrug minor ceiling ball genre embrace thunder dish guess match decide chest";
    const SECRET_KEY: &str = "51abe2873caa81e55a671d36b4ca9fda14f61f9b101d32c0eb75acf2b4e2d00d";
    const PUBLIC_KEY: &str = "b5218aa38c3883f32ebf5c9da6a0a0b57f0460a4b93e7dabc4136a599c9ee59d93514d9d99f8e3509c6649486328f415";
    const SIGNATURE: &str = "b392e56902a1ddc0b8f4785cbd0d3ed42ebfd668033a6f80e68871ac9d4b9001493ec80ce2c9663263820db35516c9d41312466b8b809da3221706460a483fe3aab2cf3c932f29d9f77f23f454cf8ef487930db500c092a8ea9fa32c698457c7";

    #[test]
    fn test_stuff() {
        // Seed
        let seed = Mnemonic::from_str(MNEMONIC).unwrap().to_seed("");

        // Secret key
        let secret_key = SecretKey::from_seed(&seed);
        let secret_key_bytes = secret_key.to_bytes();
        let secret_key_hex = secret_key_bytes.encode_hex::<String>();
        assert_eq!(secret_key_hex, SECRET_KEY);

        // Secret key round trip
        assert_eq!(
            SecretKey::from_bytes(&secret_key_bytes)
                .to_bytes()
                .encode_hex::<String>(),
            secret_key_hex
        );

        // Public key
        let public_key = secret_key.to_public_key();
        let public_key_bytes = public_key.to_bytes();
        let public_key_hex = public_key_bytes.encode_hex::<String>();
        assert_eq!(public_key_hex, PUBLIC_KEY);

        // Public key round trip
        assert_eq!(
            PublicKey::from_bytes(&public_key_bytes)
                .unwrap()
                .to_bytes()
                .encode_hex::<String>(),
            public_key_hex
        );

        // Message
        let message = b"Hello, world!";
        let mut message_hasher = Sha256::new();
        message_hasher.update(message);
        let digest = message_hasher.finalize();

        // Signature
        let signature = secret_key.sign(digest.as_slice());
        assert_eq!(signature.to_bytes().encode_hex::<String>(), SIGNATURE);
        assert!(public_key.verify(digest.as_slice(), &signature));
    }
}
