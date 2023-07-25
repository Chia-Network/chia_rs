use chia_bls::{master_to_wallet_unhardened_intermediate, DerivableKey, SecretKey};
use chia_primitives::{puzzles::DEFAULT_HIDDEN_PUZZLE_HASH, standard_puzzle_hash, DeriveSynthetic};
use indexmap::IndexMap;

pub struct KeyStore {
    intermediate_key: SecretKey,
    derivations: IndexMap<[u8; 32], SecretKey>,
}

impl KeyStore {
    pub fn new(secret_key: &SecretKey) -> Self {
        let intermediate_key = master_to_wallet_unhardened_intermediate(secret_key);

        Self {
            intermediate_key,
            derivations: IndexMap::new(),
        }
    }

    pub fn derivation_index(&self) -> u32 {
        self.derivations.len() as u32
    }

    pub fn puzzle_hashes(&self) -> Vec<[u8; 32]> {
        self.derivations.keys().cloned().collect()
    }

    pub fn secret_key_of(&self, puzzle_hash: &[u8; 32]) -> Option<&SecretKey> {
        self.derivations.get(puzzle_hash)
    }

    pub fn contains_puzzle(&self, puzzle_hash: &[u8; 32]) -> bool {
        self.derivations.contains_key(puzzle_hash)
    }

    pub fn derive_next(&mut self) -> [u8; 32] {
        let derivation_index = self.derivation_index();
        let secret_key = self.derive_key(derivation_index);
        let public_key = secret_key.to_public_key();
        let puzzle_hash = standard_puzzle_hash(&public_key);
        self.derivations.insert(puzzle_hash, secret_key);
        puzzle_hash
    }

    fn derive_key(&self, index: u32) -> SecretKey {
        self.intermediate_key
            .derive_unhardened(index)
            .derive_synthetic(&DEFAULT_HIDDEN_PUZZLE_HASH)
    }
}
