use crate::lru_cache::LRUCache;
// use crate::public_key::PublicKey;
use crate::signture::Signature;
use crate::gtelement::GTElement;
use crate::PublicKey;

type Bytes32 = [u8; 32];
type Bytes48 = [u8; 48];

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

const LOCAL_CACHE: LRUCache = LRUCache.new(50000);

// WE ARE PORTING THIS FUNCTION FROM PYTHON cached_bls.py

// def aggregate_verify(
//     pks: List[bytes48],
//     msgs: Sequence[bytes],
//     sig: G2Element,
//     force_cache: bool = False,
//     cache: LRUCache[bytes32, GTElement] = LOCAL_CACHE,
// ) -> bool:
//     pairings: List[GTElement] = get_pairings(cache, pks, msgs, force_cache)
//     if len(pairings) == 0:
//         # Using AugSchemeMPL.aggregate_verify, so it's safe to use from_bytes_unchecked
//         pks_objects: List[G1Element] = [G1Element.from_bytes_unchecked(pk) for pk in pks]
//         res: bool = AugSchemeMPL.aggregate_verify(pks_objects, msgs, sig)
//         return res

//     pairings_prod: GTElement = functools.reduce(GTElement.__mul__, pairings)
//     res = pairings_prod == sig.pair(G1Element.generator())
//     return res

// THIS IS THE PYTHON AGGREGATE_VERIFY FUNCTION IN AUGSCHEME_MPL

// pub fn aggregate_verify(pks: &PyList, msgs: &PyList, sig: &Signature) -> PyResult<bool> {
//     let mut data = Vec::<(PublicKey, Vec<u8>)>::new();
//     if pks.len() != msgs.len() {
//         return Err(PyRuntimeError::new_err(
//             "aggregate_verify expects the same number of public keys as messages",
//         ));
//     }
//     for (pk, msg) in zip(pks, msgs) {
//         let pk = pk.extract::<PublicKey>()?;
//         let msg = msg.extract::<Vec<u8>>()?;
//         data.push((pk, msg));
//     }

//     Ok(chia_bls::aggregate_verify(sig, data))
// }

fn aggregate_verify(
    pks: &[Bytes48],
    msgs: &[u8],
    sig: &Signature,
    force_cache: bool,
    cach: &mut LRUCache<bytes32, GTElement>
) {
    let pairings: [GTElement] = get_pairings(&cache, &pks, &msgs, force_cache);
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