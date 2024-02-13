extern crate lru;
use lru::LruCache;
use std::num::NonZeroUsize;
// use crate::public_key::PublicKey;
use crate::Signature;
use crate::gtelement::GTElement;
use crate::PublicKey;
use std::collections::HashMap;
use sha2::{Digest, Sha256};

pub type Bytes32 = [u8; 32];
pub type Bytes48 = [u8; 48];

pub struct BLSCache {
    cache: LruCache<Bytes32, GTElement>,
}

impl BLSCache {
    
    pub fn generator(cache_size: Option<usize>) -> Self {
        let cache: LruCache<Bytes32, GTElement> = LruCache::new(NonZeroUsize::new(cache_size.unwrap_or(50000)).unwrap());
        Self{cache}
    }
    
    // Define a function to get pairings
    fn get_pairings(
        cache: &mut LruCache<Bytes32, GTElement>,
        pks: &[Bytes48],
        msgs: &[Bytes32],
        force_cache: bool,
    ) -> Vec<GTElement> {
        let mut pairings: Vec<Option<GTElement>> = vec![];
        let mut missing_count: usize = 0;
        
        for (pk, msg) in pks.iter().zip(msgs.iter()) {
            let mut aug_msg = pk.to_vec();
            aug_msg.extend_from_slice(msg); // pk + msg
            let mut hasher = Sha256::new();
            hasher.update(aug_msg);
            let h: Bytes32 = hasher.finalize().into();
            let pairing: Option<&GTElement> = cache.get(&h);
            match pairing {
                Some(pairing) => {
                    if !force_cache {
                        // Heuristic to avoid more expensive sig validation with pairing
                        // cache when it's empty and cached pairings won't be useful later
                        // (e.g. while syncing)
                        missing_count += 1;
                        if missing_count > pks.len() / 2 {
                            return vec![];
                        }
                    }
                    pairings.push(Some(*pairing));
                },
                _ => {
                    pairings.push(None);
                },
            }
            
        }

        // G1Element.from_bytes can be expensive due to subgroup check, so we avoid recomputing it with this cache
        let mut pk_bytes_to_g1: HashMap<Bytes48, PublicKey> = HashMap::new();
        let mut ret: Vec<GTElement> = vec![];

        for (i, pairing) in pairings.iter_mut().enumerate() {
            if let Some(pairing) = pairing {
                ret.push(*pairing.clone());
            } else {
                let mut aug_msg = pks[i].to_vec();
                aug_msg.extend_from_slice(&msgs[i]);
                let aug_hash = Signature::hash_to_g2(&aug_msg);

                let pk_parsed = pk_bytes_to_g1.entry(pks[i]).or_insert_with(|| {
                    PublicKey::from_bytes(&pks[i])
                });

                let pairing = aug_hash.pair(pk_parsed);
                let h = std_hash(&aug_msg);
                cache.put(h, pairing.clone());
                ret.push(pairing);
            }
        }

        ret
    }

    fn aggregate_verify(
        self,
        pks: &[Bytes48],
        msgs: &[u8],
        sig: &Signature,
        force_cache: bool, 
    ) {
        let pairings: [GTElement] = self.get_pairings(&self.cache, &pks, &msgs, force_cache);
        if pairings.is_empty() {
            let mut data = Vec::<(PublicKey, Vec<u8>)>::new();
            for (pk, msg) in pks.iter().zip(msgs.iter()) {
                let pk = PublicKey.from_bytes_unchecked(pk);
                let msg = msg.extract::<Vec<u8>>()?;
                data.push((pk, msg));
            }
            let res: bool = Signature.aggregate_verify(sig, data);
            return res
        }
        let pairings_prod = pairings.iter().fold(GTElement, |acc, &p| acc.mul_assign(p));
        pairings_prod == sig.pair(PublicKey::generator())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn test_instantiation() {
        let bls_cache: BLSCache = BLSCache::generator(None);
    }
}

