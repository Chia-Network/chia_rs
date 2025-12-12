use crate::{Hash, Side};
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};
use chia_streamable_macro::Streamable;
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods};

#[cfg_attr(
    feature = "py-bindings",
    pyclass(get_all),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Clone, Debug, std::hash::Hash, Eq, PartialEq, Streamable)]
pub struct ProofOfInclusionLayer {
    pub other_hash_side: Side,
    pub other_hash: Hash,
    pub combined_hash: Hash,
}

#[cfg_attr(
    feature = "py-bindings",
    pyclass(get_all),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Clone, Debug, std::hash::Hash, Eq, PartialEq, Streamable)]
pub struct ProofOfInclusion {
    pub node_hash: Hash,
    pub layers: Vec<ProofOfInclusionLayer>,
}

impl ProofOfInclusion {
    pub fn root_hash(&self) -> Hash {
        if let Some(last) = self.layers.last() {
            last.combined_hash
        } else {
            self.node_hash
        }
    }

    pub fn valid(&self) -> bool {
        let mut existing_hash = self.node_hash;

        for layer in &self.layers {
            let calculated_hash = crate::calculate_internal_hash(
                &existing_hash,
                layer.other_hash_side,
                &layer.other_hash,
            );

            if calculated_hash != layer.combined_hash {
                return false;
            }

            existing_hash = calculated_hash;
        }

        existing_hash == self.root_hash()
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ProofOfInclusion {
    #[pyo3(name = "root_hash")]
    pub fn py_root_hash(&self) -> Hash {
        self.root_hash()
    }
    #[pyo3(name = "valid")]
    pub fn py_valid(&self) -> bool {
        self.valid()
    }
}

#[cfg(test)]
mod tests {
    use crate::merkle::test_util::{
        HASH_ONE, generate_hash, generate_kvid, open_dot, traversal_blob,
    };
    use crate::{Hash, KeyId, MerkleBlob, ValueId};
    use rand::SeedableRng;
    use rand::prelude::{SliceRandom, StdRng};
    use rstest::rstest;
    use std::collections::HashMap;
    use std::iter::zip;

    #[test]
    fn test_proof_of_inclusion() {
        let num_repeats = 10;
        let mut seed = 0;

        let mut random = StdRng::seed_from_u64(37);

        let mut merkle_blob = MerkleBlob::new(Vec::new()).unwrap();
        let mut keys_values: HashMap<KeyId, ValueId> = HashMap::new();

        for repeats in 0..num_repeats {
            let num_inserts = 1 + repeats * 100;
            let num_deletes = 1 + repeats * 10;

            let mut kv_ids: Vec<(KeyId, ValueId)> = Vec::new();
            let mut hashes: Vec<Hash> = Vec::new();
            for _ in 0..num_inserts {
                seed += 1;
                let (key, value) = generate_kvid(seed);
                kv_ids.push((key, value));
                hashes.push(generate_hash(seed));
                keys_values.insert(key, value);
            }

            merkle_blob
                .batch_insert(zip(kv_ids, hashes).collect())
                .unwrap();
            merkle_blob.calculate_lazy_hashes().unwrap();

            for kv_id in keys_values.keys().copied() {
                let proof_of_inclusion = match merkle_blob.get_proof_of_inclusion(kv_id) {
                    Ok(proof_of_inclusion) => proof_of_inclusion,
                    Err(error) => {
                        open_dot(merkle_blob.to_dot().unwrap().set_note(&error.to_string()));
                        panic!("here");
                    }
                };
                assert!(proof_of_inclusion.valid());
            }

            let mut delete_ordering: Vec<KeyId> = keys_values.keys().copied().collect();
            delete_ordering.shuffle(&mut random);
            delete_ordering = delete_ordering[0..num_deletes].to_vec();
            for kv_id in delete_ordering.iter().copied() {
                merkle_blob.delete(kv_id).unwrap();
                keys_values.remove(&kv_id);
            }

            for kv_id in delete_ordering {
                // with pytest.raises(Exception, match = f"unknown key: {re.escape(str(kv_id))}"):
                merkle_blob
                    .get_proof_of_inclusion(kv_id)
                    .expect_err("stuff");
            }

            let mut new_keys_values: HashMap<KeyId, ValueId> = HashMap::new();
            for old_kv in keys_values.keys().copied() {
                seed += 1;
                let (_, value) = generate_kvid(seed);
                let hash = generate_hash(seed);
                merkle_blob.upsert(old_kv, value, &hash).unwrap();
                new_keys_values.insert(old_kv, value);
            }
            if !merkle_blob.blob.is_empty() {
                merkle_blob.calculate_lazy_hashes().unwrap();
            }

            keys_values = new_keys_values;
            for kv_id in keys_values.keys().copied() {
                let proof_of_inclusion = merkle_blob.get_proof_of_inclusion(kv_id).unwrap();
                assert!(proof_of_inclusion.valid());
            }
        }
    }

    #[rstest]
    fn test_proof_of_inclusion_invalid_identified(traversal_blob: MerkleBlob) {
        let mut proof_of_inclusion = traversal_blob.get_proof_of_inclusion(KeyId(307)).unwrap();
        assert!(proof_of_inclusion.valid());
        proof_of_inclusion.layers[1].combined_hash = HASH_ONE;
        assert!(!proof_of_inclusion.valid());
    }
}
