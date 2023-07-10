use chia_bls::{master_to_wallet_unhardened_intermediate, DerivableKey, PublicKey, SecretKey};
use chia_primitives::{
    puzzles::{DEFAULT_HIDDEN_PUZZLE_HASH, P2_DELEGATED_OR_HIDDEN_HASH},
    DeriveSynthetic,
};
use clvm_utils::curry_tree_hash;
use sha2::{digest::FixedOutput, Digest, Sha256};

pub struct KeyStore {
    intermediate_key: SecretKey,
    pub derivations: Vec<Derivation>,
}

pub struct Derivation {
    pub puzzle_hash: [u8; 32],
    pub secret_key: SecretKey,
    pub public_key: PublicKey,
}

impl KeyStore {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            intermediate_key: master_to_wallet_unhardened_intermediate(&secret_key),
            derivations: Vec::new(),
        }
    }

    pub fn next_derivation_index(&self) -> u32 {
        self.derivations.len() as u32
    }

    pub fn add_next(&mut self) -> [u8; 32] {
        let child_sk = self
            .intermediate_key
            .derive_unhardened(self.next_derivation_index());

        let secret_key = child_sk.derive_synthetic(&DEFAULT_HIDDEN_PUZZLE_HASH);
        let public_key = secret_key.to_public_key();

        let mut hasher = Sha256::new();
        hasher.update([1]);
        hasher.update(public_key.to_bytes());
        let synthetic_pk_hash: [u8; 32] = hasher.finalize_fixed().into();

        let puzzle_hash = curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_pk_hash]);

        self.derivations.push(Derivation {
            puzzle_hash,
            secret_key,
            public_key,
        });

        puzzle_hash
    }

    pub fn derivation(&self, puzzle_hash: &[u8; 32]) -> Option<&Derivation> {
        self.derivations
            .iter()
            .find(|derivation| &derivation.puzzle_hash == puzzle_hash)
    }
}
