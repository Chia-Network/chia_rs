use std::borrow::Borrow;
use std::num::NonZeroUsize;

use chia_sha2::Sha256;
use lru::LruCache;
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
#[cfg_attr(feature = "py-bindings", pyo3::pyclass(name = "BLSCache"))]
#[derive(Debug)]
pub struct BlsCache {
    // sha256(pubkey + message) -> GTElement
    cache: Mutex<LruCache<[u8; 32], GTElement>>,
}

impl Default for BlsCache {
    fn default() -> Self {
        Self::new(NonZeroUsize::new(50000).unwrap())
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
    pub fn new(cache_size: NonZeroUsize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(cache_size)),
        }
    }

    pub fn len(&self) -> usize {
        self.cache.lock().expect("cache").len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().expect("cache").is_empty()
    }

    pub fn aggregate_verify<Pk: Borrow<PublicKey>, Msg: AsRef<[u8]>>(
        &mut self,
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
            if let Some(pairing) = self.cache.lock().expect("cache").get(&hash).cloned() {
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

    pub fn update(&mut self, aug_msg: &[u8], gt: GTElement) {
        let mut hasher = Sha256::new();
        hasher.update(aug_msg.as_ref());
        let hash: [u8; 32] = hasher.finalize();
        self.cache.lock().expect("cache").put(hash, gt);
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
        &mut self,
        pks: &Bound<'_, PyList>,
        msgs: &Bound<'_, PyList>,
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
        let ret = PyList::empty_bound(py);
        let c = self.cache.lock().expect("cache");
        for (key, value) in &*c {
            ret.append((PyBytes::new_bound(py, key), value.clone().into_py(py)))?;
        }
        Ok(ret.into())
    }

    #[pyo3(name = "update")]
    pub fn py_update(&mut self, other: &Bound<'_, PySequence>) -> PyResult<()> {
        let mut c = self.cache.lock().expect("cache");
        for item in other.borrow().iter()? {
            let (key, value): (Vec<u8>, GTElement) = item?.extract()?;
            c.put(
                key.try_into()
                    .map_err(|_| PyValueError::new_err("invalid key"))?,
                value,
            );
        }
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
        let mut bls_cache = BlsCache::default();

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
        let mut bls_cache = BlsCache::default();

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
        let mut bls_cache = BlsCache::new(NonZeroUsize::new(3).unwrap());

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
        assert_eq!(bls_cache.cache.lock().expect("cache").len(), 3);

        // Recreate first key.
        let sk = SecretKey::from_seed(&[1; 32]);
        let pk = sk.public_key();
        let msg = [106; 32];

        let aug_msg = [&pk.to_bytes(), msg.as_ref()].concat();

        let mut hasher = Sha256::new();
        hasher.update(aug_msg);
        let hash: [u8; 32] = hasher.finalize();

        // The first key should have been removed, since it's the oldest that's been accessed.
        assert!(!bls_cache.cache.lock().expect("cache").contains(&hash));
    }

    #[test]
    fn test_empty_sig() {
        let mut bls_cache = BlsCache::default();

        let pks_msgs: [(&PublicKey, &[u8]); 0] = [];

        assert!(bls_cache.aggregate_verify(pks_msgs, &Signature::default()));
    }
}
