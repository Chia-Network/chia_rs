
// This cache is a bit weird because it's trying to account for validating
// mempool signatures versus block signatures. When validating block signatures,
// there's not much point in caching the pairings because we're probably not going
// to see them again unless there's a reorg. However, a spend in the mempool
// is likely to reappear in a block later, so we can save having to do the pairing
// again. So caching is primarily useful after "catch-up" (fast sync?) is done and
// we're monitoring the mempool in real-time.

extern crate lru;
use crate::aggregate_verify as agg_ver;
use crate::gtelement::GTElement;
use crate::hash_to_g2;
use crate::PublicKey;
use crate::Signature;
use lru::LruCache;
use sha2::{Digest, Sha256};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::num::NonZeroUsize;

#[cfg(feature = "py-bindings")]
use pyo3::types::{PyBool, PyInt, PyList};
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods, PyResult};

pub type Bytes32 = [u8; 32];
pub type Bytes48 = [u8; 48];

#[cfg_attr(feature = "py-bindings", pyclass(name = "BLSCache"))]
pub struct BLSCache {
    cache: LruCache<Bytes32, GTElement>,
}

impl BLSCache {
    pub fn generator(cache_size: Option<usize>) -> Self {
        let cache: LruCache<Bytes32, GTElement> =
            LruCache::new(NonZeroUsize::new(cache_size.unwrap_or(50000)).unwrap());
        Self { cache }
    }

    pub fn get_pairings<P: Borrow<[Bytes48]>, M: Borrow<[Vec<u8>]>>(
        &mut self,
        pks: &P,
        msgs: &M,
        force_cache: bool,
    ) -> Vec<GTElement> {
        let mut pairings: Vec<Option<GTElement>> = vec![];
        let mut missing_count: usize = 0;

        for (pk, msg) in pks.borrow().iter().zip(msgs.borrow().iter()) {
            let mut aug_msg = pk.to_vec();
            aug_msg.extend_from_slice(msg.borrow()); // pk + msg
            let mut hasher = Sha256::new();
            hasher.update(aug_msg);
            let h: Bytes32 = hasher.finalize().into();
            let pairing: Option<&GTElement> = self.cache.get(&h);
            match pairing {
                Some(pairing) => {
                    if !force_cache {
                        // Heuristic to avoid more expensive sig validation with pairing
                        // cache when it's empty and cached pairings won't be useful later
                        // (e.g. while syncing)
                        missing_count += 1;
                        if missing_count > pks.borrow().len() / 2 {
                            return vec![];
                        }
                    }
                    pairings.push(Some(pairing.clone()));
                }
                _ => {
                    pairings.push(None);
                }
            }
        }

        // G1Element.from_bytes can be expensive due to subgroup check, so we avoid recomputing it with this cache
        let mut pk_bytes_to_g1: HashMap<Bytes48, PublicKey> = HashMap::new();
        let mut ret: Vec<GTElement> = vec![];

        for (i, pairing) in pairings.iter_mut().enumerate() {
            if let Some(pairing) = pairing {
                // equivalent to `if pairing is not None`
                ret.push(pairing.clone());
            } else {
                let mut aug_msg = pks.borrow()[i].to_vec();
                aug_msg.extend_from_slice(&msgs.borrow()[i]); // pk + msg
                let aug_hash: Signature = hash_to_g2(&aug_msg);

                let pk_parsed: &mut PublicKey = pk_bytes_to_g1
                    .entry(pks.borrow()[i])
                    .or_insert_with(|| PublicKey::from_bytes(&pks.borrow()[i]).unwrap());

                let pairing: GTElement = aug_hash.pair(pk_parsed);
                let mut hasher = Sha256::new();
                hasher.update(&aug_msg);
                let h: Bytes32 = hasher.finalize().into();
                self.cache.put(h, pairing.clone());
                ret.push(pairing);
            }
        }

        ret
    }

    pub fn aggregate_verify(
        &mut self,
        pks: &Vec<Bytes48>,
        msgs: &Vec<Vec<u8>>,
        sig: &Signature,
        force_cache: bool,
    ) -> bool {
        let mut pairings: Vec<GTElement> = self.get_pairings(pks, msgs, force_cache);
        if pairings.is_empty() {
            let mut data = Vec::<(PublicKey, Vec<u8>)>::new();
            for (pk, msg) in pks.iter().zip(msgs.iter()) {
                let pk = PublicKey::from_bytes_unchecked(pk).unwrap();
                data.push((pk.clone(), msg.clone()));
            }
            let res: bool = agg_ver(sig, data);
            return res;
        }
        let pairings_prod = pairings.pop(); // start with the first pairing
        match pairings_prod {
            Some(mut prod) => {
                for p in pairings.iter() {
                    // loop through rest of list
                    prod *= &p;
                }
                prod == sig.pair(&PublicKey::generator())
            }
            _ => pairings.is_empty(),
        }
    }
}

// Python Functions
#[cfg(feature = "py-bindings")]
#[pymethods]
impl BLSCache {
    #[new]
    pub fn init() -> Self {
        Self::generator(None)
    }

    #[staticmethod]
    #[pyo3(name = "generator")]
    pub fn py_generator(size: Option<&PyInt>) -> Self {
        match size {
            Some(s) => {
                let usize_value: usize = s.extract::<usize>().unwrap();
                Self::generator(Some(usize_value))
            }
            None => Self::generator(None),
        }
    }

    #[pyo3(name = "aggregate_verify")]
    pub fn py_aggregate_verify(
        &mut self,
        pks: &PyList,
        msgs: &PyList,
        sig: &Signature,
        force_cache: &PyBool,
    ) -> PyResult<bool> {
        let pks_r: Vec<Bytes48> = pks
            .iter()
            .map(|item| item.extract::<Bytes48>())
            .collect::<PyResult<_>>()?;
        let msgs_r: Vec<Vec<u8>> = msgs
            .iter()
            .map(|item| item.extract::<Vec<u8>>())
            .collect::<PyResult<_>>()?;
        let force_cache_bool = force_cache.extract::<bool>()?;
        Ok(self.aggregate_verify(&pks_r, &msgs_r, sig, force_cache_bool))
    }

    #[pyo3(name = "len")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.cache.len())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::aggregate;
    use crate::sign;
    use crate::SecretKey;

    #[test]
    pub fn test_instantiation() {
        let mut bls_cache: BLSCache = BLSCache::generator(None);
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
        let h: Bytes32 = hasher.finalize().into();
        bls_cache.cache.put(h, pairing.clone());
        assert_eq!(*bls_cache.cache.get(&h).unwrap(), pairing);
    }

    #[test]
    pub fn test_aggregate_verify() {
        let mut bls_cache: BLSCache = BLSCache::generator(None);
        assert_eq!(bls_cache.cache.len(), 0);
        let byte_array: [u8; 32] = [0; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: Vec<u8> = [106; 32].to_vec();
        let sig: Signature = sign(&sk, &msg);
        let pk_list: Vec<[u8; 48]> = [pk.to_bytes()].to_vec();
        let msg_list: Vec<Vec<u8>> = [msg].to_vec();
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, true));
        assert_eq!(bls_cache.cache.len(), 1);
        // try again with (pk, msg) cached
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, true));
        assert_eq!(bls_cache.cache.len(), 1);
    }

    #[test]
    pub fn test_cache() {
        let mut bls_cache: BLSCache = BLSCache::generator(None);
        assert_eq!(bls_cache.cache.len(), 0);
        let byte_array: [u8; 32] = [0; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: Vec<u8> = [106; 32].to_vec();
        let sig: Signature = sign(&sk, &msg);
        let mut pk_list: Vec<[u8; 48]> = [pk.to_bytes()].to_vec();
        let mut msg_list: Vec<Vec<u8>> = [msg].to_vec();
        // add first to cache
        // try one cached, one not cached
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, false));
        assert_eq!(bls_cache.cache.len(), 1);
        let byte_array: [u8; 32] = [1; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: Vec<u8> = [107; 32].to_vec();
        let sig = aggregate([sig, sign(&sk, &msg)]);
        pk_list.push(pk.to_bytes());
        msg_list.push(msg);
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, false));
        assert_eq!(bls_cache.cache.len(), 2);
        // try reusing a pubkey
        let pk: PublicKey = sk.public_key();
        let msg: Vec<u8> = [108; 32].to_vec();
        let sig = aggregate([sig, sign(&sk, &msg)]);
        pk_list.push(pk.to_bytes());
        msg_list.push(msg);
        // try with force_cache disabled
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, false));
        assert_eq!(bls_cache.cache.len(), 2);
        // now force it to save the pairing
        assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, true));
        assert_eq!(bls_cache.cache.len(), 3);
    }

    #[test]
    pub fn test_cache_limit() {
        // set cache size to 3
        let mut bls_cache: BLSCache = BLSCache::generator(Some(3));
        assert_eq!(bls_cache.cache.len(), 0);
        // create 5 pk/msg combos
        for i in 1..=5 {
            let byte_array: [u8; 32] = [i as u8; 32];
            let sk: SecretKey = SecretKey::from_seed(&byte_array);
            let pk: PublicKey = sk.public_key();
            let msg: Vec<u8> = [106; 32].to_vec();
            let sig: Signature = sign(&sk, &msg);
            let pk_list: Vec<[u8; 48]> = [pk.to_bytes()].to_vec();
            let msg_list: Vec<Vec<u8>> = [msg].to_vec();
            assert!(bls_cache.aggregate_verify(&pk_list, &msg_list, &sig, true));
        }
        assert_eq!(bls_cache.cache.len(), 3);
        // recreate first key
        let byte_array: [u8; 32] = [1; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: Vec<u8> = [106; 32].to_vec();
        let mut aug_msg = pk.to_bytes().to_vec();
        aug_msg.extend_from_slice(&msg); // pk + msg
        let mut hasher = Sha256::new();
        hasher.update(aug_msg);
        let h: Bytes32 = hasher.finalize().into();
        // assert first key has been removed
        assert!(bls_cache.cache.get(&h).is_none());
    }
}