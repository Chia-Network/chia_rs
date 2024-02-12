use crate::lru_cache::LRUCache;
// use crate::public_key::PublicKey;
use crate::signture::Signature;
use crate::gtelement::GTElement;
use crate::PublicKey;

type Bytes32 = [u8; 32];
type Bytes48 = [u8; 48];

pub struct BLSCache {
    pub cache: LRUCache<Bytes32, GTElement>,
}

impl BLSCache {
    
    pub fn generator(cache_size: Option<u128>) -> Self {
        let cache: LRUCache = LRUCache.new(cache_size.unwrap_or(50000));
        Self(cache)
    }
    
    // Define a function to get pairings
    fn get_pairings(
        cache: &mut LRUCache<Bytes32, GTElement>,
        pks: &[Bytes48],
        msgs: &[Bytes32],
        force_cache: bool,
    ) -> Vec<GTElement> {
        let mut pairings: Vec<Option<GTElement>> = vec![];
        let mut missing_count: usize = 0;

        for (pk, msg) in pks.iter().zip(msgs.iter()) {
            let mut aug_msg = pk.to_vec();
            aug_msg.extend_from_slice(msg);
            let h = std_hash(&aug_msg);

            if let Some(pairing) = cache.get(&h) {
                if !force_cache && pairing.is_none() {
                    // Heuristic to avoid more expensive sig validation with pairing
                    // cache when it's empty and cached pairings won't be useful later
                    // (e.g. while syncing)
                    missing_count += 1;
                    if missing_count > pks.len() / 2 {
                        return vec![];
                    }
                }
            }

            pairings.push(cache.get(&h).cloned());
        }

        // G1Element.from_bytes can be expensive due to subgroup check, so we avoid recomputing it with this cache
        let mut pk_bytes_to_g1: HashMap<Bytes48, PublicKey> = HashMap::new();
        let mut ret: Vec<GTElement> = vec![];

        for (i, pairing) in pairings.iter_mut().enumerate() {
            if let Some(pairing) = pairing {
                ret.push(pairing.clone());
            } else {
                let mut aug_msg = pks[i].to_vec();
                aug_msg.extend_from_slice(&msgs[i]);
                let aug_hash = G2Element::from_message(&aug_msg);

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
        pks: &[Bytes48],
        msgs: &[u8],
        sig: &Signature,
        force_cache: bool, 
    ) {
        let pairings: [GTElement] = get_pairings(&self.cache, &pks, &msgs, force_cache);
        if pairings.is_empty() {
            let mut data = Vec::<(PublicKey, Vec<u8>)>::new();
            for (pks, msg) in zip(pks, msgs) {
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

