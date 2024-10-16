use crate::secret_key::SecretKey;

pub trait DerivableKey {
    #[must_use]
    fn derive_unhardened(&self, idx: u32) -> Self;
}

fn derive_path_unhardened<Key: DerivableKey>(key: &Key, path: &[u32]) -> Key {
    let mut derived = key.derive_unhardened(path[0]);
    for idx in &path[1..] {
        derived = derived.derive_unhardened(*idx);
    }
    derived
}

fn derive_path_hardened(key: &SecretKey, path: &[u32]) -> SecretKey {
    let mut derived = key.derive_hardened(path[0]);
    for idx in &path[1..] {
        derived = derived.derive_hardened(*idx);
    }
    derived
}

pub fn master_to_wallet_unhardened_intermediate<Key: DerivableKey>(key: &Key) -> Key {
    derive_path_unhardened(key, &[12381_u32, 8444, 2])
}

pub fn master_to_wallet_unhardened<Key: DerivableKey>(key: &Key, idx: u32) -> Key {
    derive_path_unhardened(key, &[12381_u32, 8444, 2, idx])
}

pub fn master_to_wallet_hardened_intermediate(key: &SecretKey) -> SecretKey {
    derive_path_hardened(key, &[12381_u32, 8444, 2])
}

pub fn master_to_wallet_hardened(key: &SecretKey, idx: u32) -> SecretKey {
    derive_path_hardened(key, &[12381_u32, 8444, 2, idx])
}

pub fn master_to_pool_singleton(key: &SecretKey, pool_wallet_idx: u32) -> SecretKey {
    derive_path_hardened(key, &[12381_u32, 8444, 5, pool_wallet_idx])
}

/// # Panics
///
/// Panics if `pool_wallet_idx` or `idx` is greater than or equal to 10000.
pub fn master_to_pool_authentication(key: &SecretKey, pool_wallet_idx: u32, idx: u32) -> SecretKey {
    assert!(pool_wallet_idx < 10000);
    assert!(idx < 10000);
    derive_path_hardened(key, &[12381_u32, 8444, 6, pool_wallet_idx * 10000 + idx])
}
