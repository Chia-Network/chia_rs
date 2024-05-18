use std::borrow::Borrow;
use std::num::NonZeroUsize;

use lru::LruCache;
use sha2::{Digest, Sha256};

use crate::{aggregate_verify_gt, hash_to_g2};
use crate::{GTElement, PublicKey, Signature};

/// This cache is a bit weird because it's trying to account for validating
/// mempool signatures versus block signatures. When validating block signatures,
/// there's not much point in caching the pairings because we're probably not going
/// to see them again unless there's a reorg. However, a spend in the mempool
/// is likely to reappear in a block later, so we can save having to do the pairing
/// again. So caching is primarily useful when synced and monitoring the mempool in real-time.
#[cfg_attr(feature = "py-bindings", pyo3::pyclass(name = "BLSCache"))]
#[derive(Clone)]
pub struct BlsCache {
    // sha256(pubkey + message) -> GTElement
    cache: LruCache<[u8; 32], GTElement>,
}

impl Default for BlsCache {
    fn default() -> Self {
        Self::new(NonZeroUsize::new(50000).unwrap())
    }
}

impl BlsCache {
    pub fn new(cache_size: NonZeroUsize) -> Self {
        Self {
            cache: LruCache::new(cache_size),
        }
    }

    pub fn aggregate_verify(
        &mut self,
        pks: impl IntoIterator<Item = impl Borrow<PublicKey>>,
        msgs: impl IntoIterator<Item = impl AsRef<[u8]>>,
        sig: &Signature,
    ) -> bool {
        let iter = pks.into_iter().zip(msgs).map(|(pk, msg)| -> GTElement {
            // Hash pubkey + message
            let mut hasher = Sha256::new();
            hasher.update(pk.borrow().to_bytes());
            hasher.update(msg.as_ref());
            let hash: [u8; 32] = hasher.finalize().into();

            // If the pairing is in the cache, we don't need to recalculate it.
            if let Some(pairing) = self.cache.get(&hash).cloned() {
                return pairing;
            }

            // Calculate the pairing and add it to the cache.
            let mut aug_msg = pk.borrow().to_bytes().to_vec();
            aug_msg.extend_from_slice(msg.as_ref());
            let aug_hash: Signature = hash_to_g2(&aug_msg);

            let pairing: GTElement = aug_hash.pair(pk.borrow());
            let mut hasher = Sha256::new();
            hasher.update(&aug_msg);
            let h: [u8; 32] = hasher.finalize().into();
            self.cache.put(h, pairing.clone());
            pairing
        });
        aggregate_verify_gt(sig, iter)
    }
}

#[cfg(feature = "py-bindings")]
mod python {
    use super::*;

    use pyo3::{
        exceptions::PyValueError,
        pybacked::PyBackedBytes,
        pymethods,
        types::{PyAnyMethods, PyList},
        Bound, PyResult,
    };

    #[pymethods]
    impl BlsCache {
        #[new]
        pub fn init(size: Option<u32>) -> PyResult<Self> {
            let Some(size) = size else {
                return Ok(Self::default());
            };

            let Some(size) = NonZeroUsize::new(size as usize) else {
                return Err(PyValueError::new_err(
                    "Cannot have a cache size less than one.",
                ));
            };

            Ok(Self::new(size))
        }

        #[pyo3(name = "aggregate_verify")]
        pub fn py_aggregate_verify(
            &mut self,
            pks: &Bound<PyList>,
            msgs: &Bound<PyList>,
            sig: &Signature,
        ) -> PyResult<bool> {
            let pks = pks
                .iter()?
                .map(|item| item?.extract())
                .collect::<PyResult<Vec<PublicKey>>>()?;

            let msgs = msgs
                .iter()?
                .map(|item| item?.extract())
                .collect::<PyResult<Vec<PyBackedBytes>>>()?;

            Ok(self.aggregate_verify(pks, msgs, sig))
        }

        #[pyo3(name = "len")]
        pub fn py_len(&self) -> PyResult<usize> {
            Ok(self.cache.len())
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::aggregate;
    use crate::sign;
    use crate::SecretKey;

    #[test]
    fn test_instantiation() {
        let mut bls_cache: BlsCache = BlsCache::default();
        let byte_array: [u8; 32] = [0; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: [u8; 32] = [106; 32];
        let mut aug_msg: Vec<u8> = pk.clone().to_bytes().to_vec();
        aug_msg.extend_from_slice(&msg); // pk + msg
        let aug_hash = hash_to_g2(&aug_msg);
        let pairing = aug_hash.pair(&pk);
        let mut hasher = Sha256::new();
        hasher.update(&aug_msg);
        let h: [u8; 32] = hasher.finalize().into();
        bls_cache.cache.put(h, pairing.clone());
        assert_eq!(*bls_cache.cache.get(&h).unwrap(), pairing);
    }

    #[test]
    fn test_aggregate_verify() {
        let mut bls_cache: BlsCache = BlsCache::default();
        assert_eq!(bls_cache.cache.len(), 0);
        let byte_array: [u8; 32] = [0; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: &[u8] = &[106; 32];
        let sig: Signature = sign(&sk, msg);
        let pk_list: Vec<PublicKey> = [pk].to_vec();
        let msg_list: Vec<&[u8]> = [msg].to_vec();
        assert!(bls_cache.aggregate_verify(pk_list.iter(), msg_list.iter(), &sig));
        assert_eq!(bls_cache.cache.len(), 1);
        // try again with (pk, msg) cached
        assert!(bls_cache.aggregate_verify(pk_list, msg_list, &sig));
        assert_eq!(bls_cache.cache.len(), 1);
    }

    #[test]
    fn test_cache() {
        let mut bls_cache: BlsCache = BlsCache::default();
        assert_eq!(bls_cache.cache.len(), 0);
        let byte_array: [u8; 32] = [0; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: &[u8] = &[106; 32];
        let sig: Signature = sign(&sk, msg);
        let mut pk_list: Vec<PublicKey> = [pk].to_vec();
        let mut msg_list: Vec<&[u8]> = [msg].to_vec();
        // add first to cache
        // try one cached, one not cached
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig));
        assert_eq!(bls_cache.cache.len(), 1);
        let byte_array: [u8; 32] = [1; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: &[u8] = &[107; 32];
        let sig = aggregate([sig, sign(&sk, msg)]);
        pk_list.push(pk);
        msg_list.push(msg);
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig));
        assert_eq!(bls_cache.cache.len(), 2);
        // try reusing a pubkey
        let pk: PublicKey = sk.public_key();
        let msg: &[u8] = &[108; 32];
        let sig = aggregate([sig, sign(&sk, msg)]);
        pk_list.push(pk);
        msg_list.push(msg);
        // check verification
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig));
        assert_eq!(bls_cache.cache.len(), 3);
    }

    #[test]
    fn test_cache_limit() {
        // set cache size to 3
        let mut bls_cache: BlsCache = BlsCache::new(NonZeroUsize::new(3).unwrap());
        assert_eq!(bls_cache.cache.len(), 0);
        // create 5 pk/msg combos
        for i in 1..=5 {
            let byte_array: [u8; 32] = [i as u8; 32];
            let sk: SecretKey = SecretKey::from_seed(&byte_array);
            let pk: PublicKey = sk.public_key();
            let msg: &[u8] = &[106; 32];
            let sig: Signature = sign(&sk, msg);
            let pk_list: Vec<PublicKey> = [pk].to_vec();
            let msg_list: Vec<&[u8]> = vec![msg];
            // add to cache by validating them one at a time
            assert!(bls_cache.aggregate_verify(pk_list.iter(), msg_list.iter(), &sig));
        }
        assert_eq!(bls_cache.cache.len(), 3);
        // recreate first key
        let byte_array: [u8; 32] = [1; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: Vec<u8> = vec![106; 32];
        let mut aug_msg = pk.to_bytes().to_vec();
        aug_msg.extend_from_slice(&msg); // pk + msg
        let mut hasher = Sha256::new();
        hasher.update(aug_msg);
        let h: [u8; 32] = hasher.finalize().into();
        // assert first key has been removed
        assert!(bls_cache.cache.get(&h).is_none());
    }

    #[test]
    fn test_empty_sig() {
        let mut bls_cache: BlsCache = BlsCache::default();
        let sig: Signature = aggregate(&[]);
        let pk_list: [PublicKey; 0] = [];
        let msg_list: Vec<&[u8]> = vec![];
        assert!(bls_cache.aggregate_verify(pk_list, msg_list, &sig));
    }
}
