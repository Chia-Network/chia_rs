use std::sync::Arc;

use chia_bls::{master_to_wallet_unhardened_intermediate, DerivableKey, SecretKey};
use chia_primitives::{
    puzzles::{DEFAULT_HIDDEN_PUZZLE_HASH, P2_DELEGATED_OR_HIDDEN_HASH},
    DeriveSynthetic,
};
use clvm_utils::{curry_tree_hash, tree_hash_atom};
use indexmap::IndexMap;

pub struct KeyStore {
    intermediate_key: SecretKey,
    derivations: IndexMap<[u8; 32], Arc<SecretKey>>,
}

impl KeyStore {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            intermediate_key: master_to_wallet_unhardened_intermediate(&secret_key),
            derivations: IndexMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.derivations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.derivations.is_empty()
    }

    pub fn puzzle_hashes(&self) -> Vec<[u8; 32]> {
        self.derivations.keys().copied().collect()
    }

    pub fn contains(&self, puzzle_hash: &[u8; 32]) -> bool {
        self.derivations.contains_key(puzzle_hash)
    }

    pub fn derivation(&self, puzzle_hash: &[u8; 32]) -> Option<Arc<SecretKey>> {
        self.derivations.get(puzzle_hash).map(Arc::clone)
    }

    pub fn derive_next(&mut self) -> [u8; 32] {
        let child_sk = self.intermediate_key.derive_unhardened(self.len() as u32);

        let secret_key = child_sk.derive_synthetic(&DEFAULT_HIDDEN_PUZZLE_HASH);
        let public_key = secret_key.to_public_key();

        let puzzle_hash = curry_tree_hash(
            &P2_DELEGATED_OR_HIDDEN_HASH,
            &[&tree_hash_atom(&public_key.to_bytes())],
        );

        self.derivations.insert(puzzle_hash, Arc::new(secret_key));
        puzzle_hash
    }
}
