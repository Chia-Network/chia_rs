use std::collections::HashMap;

use chia_bls::{PublicKey, SecretKey};

pub struct KeyStore {
    secret_key: SecretKey,
    public_key: PublicKey,
    derivations: HashMap<u32, Derivation>,
}

pub struct Derivation {
    secret_key: SecretKey,
    public_key: PublicKey,
    puzzle_hash: [u8; 32],
}

impl KeyStore {
    pub fn new(secret_key: SecretKey) -> Self {
        let public_key = secret_key.to_public_key();

        Self {
            secret_key,
            public_key,
            derivations: HashMap::new(),
        }
    }
}
