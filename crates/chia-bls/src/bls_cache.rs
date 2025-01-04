use std::borrow::Borrow;
use std::num::NonZeroUsize;

use chia_sha2::Sha256;
use linked_hash_map::LinkedHashMap;
use std::sync::Mutex;

use crate::{aggregate_verify_gt, hash_to_g2};
use crate::{GTElement, PublicKey, Signature};

/// This is a cache of pairings of public keys and their corresponding message.
/// It accelerates aggregate verification when some public keys have already
/// been paired, and found in the cache.
/// We use it to cache pairings when validating transactions inserted into the
/// mempool, as many of those transactions are likely to show up in a full block
/// later. This makes it a lot cheaper to validate the full block.
/// However, validating a signature where we have no cached GT elements, the
/// aggregate_verify() primitive is faster. When long-syncing, that's
/// preferable.

#[derive(Debug, Clone)]
struct BlsCacheData {
    // sha256(pubkey + message) -> GTElement
    items: LinkedHashMap<[u8; 32], GTElement>,
    capacity: NonZeroUsize,
}

impl BlsCacheData {
    pub fn put(&mut self, hash: [u8; 32], pairing: GTElement) {
        // If the cache is full, remove the oldest item.
        if self.items.len() == self.capacity.get() {
            if let Some((oldest_key, _)) = self.items.pop_front() {
                self.items.remove(&oldest_key);
            }
        }
        self.items.insert(hash, pairing);
    }
}

#[cfg_attr(feature = "py-bindings", pyo3::pyclass(name = "BLSCache"))]
#[derive(Debug)]
pub struct BlsCache {
    cache: Mutex<BlsCacheData>,
}

impl Default for BlsCache {
    fn default() -> Self {
        Self::new(NonZeroUsize::new(50_000).unwrap())
    }
}

impl Clone for BlsCache {
    fn clone(&self) -> Self {
        Self {
            cache: Mutex::new(self.cache.lock().expect("cache").clone()),
        }
    }
}

impl BlsCache {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            cache: Mutex::new(BlsCacheData {
                items: LinkedHashMap::new(),
                capacity,
            }),
        }
    }

    pub fn len(&self) -> usize {
        self.cache.lock().expect("cache").items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().expect("cache").items.is_empty()
    }

    pub fn aggregate_verify<Pk: Borrow<PublicKey>, Msg: AsRef<[u8]>>(
        &self,
        pks_msgs: impl IntoIterator<Item = (Pk, Msg)>,
        sig: &Signature,
    ) -> bool {
        let iter = pks_msgs.into_iter().map(|(pk, msg)| -> GTElement {
            // Hash pubkey + message
            let mut hasher = Sha256::new();
            let mut aug_msg = pk.borrow().to_bytes().to_vec();
            aug_msg.extend_from_slice(msg.as_ref());
            hasher.update(&aug_msg);
            let hash: [u8; 32] = hasher.finalize();

            // If the pairing is in the cache, we don't need to recalculate it.
            if let Some(pairing) = self.cache.lock().expect("cache").items.get(&hash).cloned() {
                return pairing;
            }

            // Otherwise, we need to calculate the pairing and add it to the cache.
            let aug_hash = hash_to_g2(&aug_msg);

            let pairing = aug_hash.pair(pk.borrow());
            self.cache.lock().expect("cache").put(hash, pairing.clone());
            pairing
        });

        aggregate_verify_gt(sig, iter)
    }

    pub fn update(&self, aug_msg: &[u8], gt: GTElement) {
        let mut hasher = Sha256::new();
        hasher.update(aug_msg.as_ref());
        let hash: [u8; 32] = hasher.finalize();
        self.cache.lock().expect("cache").put(hash, gt);
    }

    pub fn evict<Pk, Msg>(&self, pks_msgs: impl IntoIterator<Item = (Pk, Msg)>)
    where
        Pk: Borrow<PublicKey>,
        Msg: AsRef<[u8]>,
    {
        let mut c = self.cache.lock().expect("cache");
        for (pk, msg) in pks_msgs {
            let mut hasher = Sha256::new();
            let mut aug_msg = pk.borrow().to_bytes().to_vec();
            aug_msg.extend_from_slice(msg.as_ref());
            hasher.update(&aug_msg);
            let hash: [u8; 32] = hasher.finalize();
            c.items.remove(&hash);
        }
    }
}

#[cfg(feature = "py-bindings")]
use pyo3::{
    exceptions::PyValueError,
    pybacked::PyBackedBytes,
    types::{PyAnyMethods, PyList, PySequence},
    Bound, PyObject, PyResult,
};

#[cfg(feature = "py-bindings")]
#[pyo3::pymethods]
impl BlsCache {
    #[new]
    #[pyo3(signature = (size=None))]
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
        &self,
        pks: &Bound<'_, PyList>,
        msgs: &Bound<'_, PyList>,
        sig: &Signature,
    ) -> PyResult<bool> {
        let pks = pks
            .try_iter()?
            .map(|item| item?.extract())
            .collect::<PyResult<Vec<PublicKey>>>()?;

        let msgs = msgs
            .try_iter()?
            .map(|item| item?.extract())
            .collect::<PyResult<Vec<PyBackedBytes>>>()?;

        Ok(self.aggregate_verify(pks.into_iter().zip(msgs), sig))
    }

    #[pyo3(name = "len")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.len())
    }

    #[pyo3(name = "items")]
    pub fn py_items(&self, py: pyo3::Python<'_>) -> PyResult<PyObject> {
        use pyo3::prelude::*;
        use pyo3::types::PyBytes;
        let ret = PyList::empty(py);
        let c = self.cache.lock().expect("cache");
        for (key, value) in &c.items {
            ret.append((
                PyBytes::new(py, key),
                value.clone().into_pyobject(py)?.into_any(),
            ))?;
        }
        Ok(ret.into())
    }

    #[pyo3(name = "update")]
    pub fn py_update(&self, other: &Bound<'_, PySequence>) -> PyResult<()> {
        let mut c = self.cache.lock().expect("cache");
        for item in other.borrow().try_iter()? {
            let (key, value): (Vec<u8>, GTElement) = item?.extract()?;
            c.put(
                key.try_into()
                    .map_err(|_| PyValueError::new_err("invalid key"))?,
                value,
            );
        }
        Ok(())
    }

    #[pyo3(name = "evict")]
    pub fn py_evict(&self, pks: &Bound<'_, PyList>, msgs: &Bound<'_, PyList>) -> PyResult<()> {
        let pks = pks
            .try_iter()?
            .map(|item| item?.extract())
            .collect::<PyResult<Vec<PublicKey>>>()?;
        let msgs = msgs
            .try_iter()?
            .map(|item| item?.extract())
            .collect::<PyResult<Vec<PyBackedBytes>>>()?;
        self.evict(pks.into_iter().zip(msgs));
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use crate::sign;
    use crate::SecretKey;

    #[test]
    fn test_aggregate_verify() {
        let bls_cache = BlsCache::default();

        let sk = SecretKey::from_seed(&[0; 32]);
        let pk = sk.public_key();
        let msg = [106; 32];

        let sig = sign(&sk, msg);
        let pks_msgs = [(pk, msg)];

        // Before we cache anything, it should be empty.
        assert!(bls_cache.is_empty());

        // Verify the signature and add to the cache.
        assert!(bls_cache.aggregate_verify(pks_msgs, &sig));
        assert_eq!(bls_cache.len(), 1);

        // Now that it's cached, it shouldn't cache it again.
        assert!(bls_cache.aggregate_verify(pks_msgs, &sig));
        assert_eq!(bls_cache.len(), 1);
    }

    #[test]
    fn test_cache() {
        let bls_cache = BlsCache::default();

        let sk1 = SecretKey::from_seed(&[0; 32]);
        let pk1 = sk1.public_key();
        let msg1 = [106; 32];

        let mut agg_sig = sign(&sk1, msg1);
        let mut pks_msgs = vec![(pk1, msg1)];

        // Before we cache anything, it should be empty.
        assert!(bls_cache.is_empty());

        // Add the first signature to cache.
        assert!(bls_cache.aggregate_verify(pks_msgs.clone(), &agg_sig));
        assert_eq!(bls_cache.len(), 1);

        // Try with the first key message pair in the cache but not the second.
        let sk2 = SecretKey::from_seed(&[1; 32]);
        let pk2 = sk2.public_key();
        let msg2 = [107; 32];

        agg_sig += &sign(&sk2, msg2);
        pks_msgs.push((pk2, msg2));

        assert!(bls_cache.aggregate_verify(pks_msgs.clone(), &agg_sig));
        assert_eq!(bls_cache.len(), 2);

        // Try reusing a public key.
        let msg3 = [108; 32];

        agg_sig += &sign(&sk2, msg3);
        pks_msgs.push((pk2, msg3));

        // Verify this signature and add to the cache as well (since it's still a different aggregate).
        assert!(bls_cache.aggregate_verify(pks_msgs, &agg_sig));
        assert_eq!(bls_cache.len(), 3);
    }

    #[test]
    fn test_cache_limit() {
        // The cache is limited to only 3 items.
        let bls_cache = BlsCache::new(NonZeroUsize::new(3).unwrap());

        // Before we cache anything, it should be empty.
        assert!(bls_cache.is_empty());

        // Create 5 pubkey message pairs.
        for i in 1..=5 {
            let sk = SecretKey::from_seed(&[i; 32]);
            let pk = sk.public_key();
            let msg = [106; 32];

            let sig = sign(&sk, msg);
            let pks_msgs = [(pk, msg)];

            // Add to cache by validating them one at a time.
            assert!(bls_cache.aggregate_verify(pks_msgs, &sig));
        }

        // The cache should be full now.
        assert_eq!(bls_cache.len(), 3);

        // Recreate first two keys and make sure they got removed.
        for i in 1..=2 {
            let sk = SecretKey::from_seed(&[i; 32]);
            let pk = sk.public_key();
            let msg = [106; 32];
            let aug_msg = [&pk.to_bytes(), msg.as_ref()].concat();
            let mut hasher = Sha256::new();
            hasher.update(aug_msg);
            let hash: [u8; 32] = hasher.finalize();
            assert!(!bls_cache
                .cache
                .lock()
                .expect("cache")
                .items
                .contains_key(&hash));
        }
    }

    #[test]
    fn test_empty_sig() {
        let bls_cache = BlsCache::default();

        let pks_msgs: [(&PublicKey, &[u8]); 0] = [];

        assert!(bls_cache.aggregate_verify(pks_msgs, &Signature::default()));
    }

    #[test]
    fn test_evict() {
        let bls_cache = BlsCache::new(NonZeroUsize::new(5).unwrap());
        // Create 5 pk msg pairs and add them to the cache.
        let mut pks_msgs = Vec::new();
        for i in 1..=5 {
            let sk = SecretKey::from_seed(&[i; 32]);
            let pk = sk.public_key();
            let msg = [42; 32];
            let sig = sign(&sk, msg);
            pks_msgs.push((pk, msg));
            assert!(bls_cache.aggregate_verify([(pk, msg)], &sig));
        }
        assert_eq!(bls_cache.len(), 5);
        // Evict the first and third entries.
        let pks_msgs_to_evict = vec![pks_msgs[0], pks_msgs[2]];
        bls_cache.evict(pks_msgs_to_evict.iter().copied());
        // The cache should have 3 items now.
        assert_eq!(bls_cache.len(), 3);
        // Check that the evicted entries are no longer in the cache.
        for (pk, msg) in &pks_msgs_to_evict {
            let aug_msg = [&pk.to_bytes(), msg.as_ref()].concat();
            let mut hasher = Sha256::new();
            hasher.update(aug_msg);
            let hash: [u8; 32] = hasher.finalize();
            assert!(!bls_cache
                .cache
                .lock()
                .expect("cache")
                .items
                .contains_key(&hash));
        }
        // Check that the remaining entries are still in the cache.
        for (pk, msg) in &[pks_msgs[1], pks_msgs[3], pks_msgs[4]] {
            let aug_msg = [&pk.to_bytes(), msg.as_ref()].concat();
            let mut hasher = Sha256::new();
            hasher.update(aug_msg);
            let hash: [u8; 32] = hasher.finalize();
            assert!(bls_cache
                .cache
                .lock()
                .expect("cache")
                .items
                .contains_key(&hash));
        }
    }
}
