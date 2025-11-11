use crate::merkle::error::Error;
use crate::{
    Hash, InternalNodesMap, KeyId, LeafNodesMap, MerkleBlob, Node, NodeHashToDeltaReaderNode,
    NodeHashToIndex, ParentFirstIterator, TreeIndex, ValueId,
};
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods, PyResult, Python};
use rayon::iter::{IntoParallelIterator, ParallelExtend, ParallelIterator};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub enum DeltaReaderNode {
    Internal { left: Hash, right: Hash },
    Leaf { key: KeyId, value: ValueId },
}

#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct DeltaFileCache {
    hash_to_index: NodeHashToIndex,
    previous_hashes: HashSet<Hash>,
    merkle_blob: MerkleBlob,
}

impl DeltaFileCache {
    pub fn new(path: &PathBuf) -> Result<Self, Error> {
        let merkle_blob = MerkleBlob::from_path(path)?;
        let hash_to_index = merkle_blob.get_hashes_indexes(false)?;
        Ok(Self {
            hash_to_index,
            previous_hashes: HashSet::new(),
            merkle_blob,
        })
    }

    pub fn load_previous_hashes(&mut self, path: &PathBuf) -> Result<(), Error> {
        let blob = crate::zstd_decode_path(path)?;
        self.previous_hashes = HashSet::new();

        if !blob.is_empty() {
            for item in ParentFirstIterator::new(&blob, None) {
                let (_, block) = item?;
                self.previous_hashes.insert(block.node.hash());
            }
        }
        Ok(())
    }

    pub fn get_raw_node(&self, index: TreeIndex) -> Result<Node, Error> {
        self.merkle_blob.get_node(index)
    }

    pub fn get_hash_at_index(&self, index: TreeIndex) -> Result<Option<Hash>, Error> {
        self.merkle_blob.get_hash_at_index(index)
    }

    pub fn seen_previous_hash(&self, hash: Hash) -> bool {
        self.previous_hashes.contains(&hash)
    }

    pub fn get_index(&self, hash: Hash) -> Result<TreeIndex, Error> {
        self.hash_to_index
            .get(&hash)
            .copied()
            .ok_or(Error::HashNotFound(hash))
    }
}

#[cfg_attr(feature = "py-bindings", pyclass)]
pub struct DeltaReader {
    nodes: NodeHashToDeltaReaderNode,
}

impl DeltaReader {
    pub fn new(internal_nodes: InternalNodesMap, leaf_nodes: LeafNodesMap) -> Result<Self, Error> {
        let mut nodes = NodeHashToDeltaReaderNode::new();

        for (hash, (left, right)) in internal_nodes {
            nodes.insert(hash, DeltaReaderNode::Internal { left, right });
        }
        for (hash, (key, value)) in leaf_nodes {
            nodes.insert(hash, DeltaReaderNode::Leaf { key, value });
        }

        Ok(Self { nodes })
    }

    pub fn get_missing_hashes(&self, root_hash: Hash) -> HashSet<Hash> {
        let mut missing_hashes: HashSet<Hash> = HashSet::new();

        for node in self.nodes.values() {
            let DeltaReaderNode::Internal { left, right } = node else {
                continue;
            };

            for hash in [left, right] {
                if !self.nodes.contains_key(hash) {
                    missing_hashes.insert(*hash);
                }
            }
        }

        if !self.nodes.contains_key(&root_hash) {
            missing_hashes.insert(root_hash);
        }

        missing_hashes
    }

    pub fn collect_from_merkle_blob(
        &mut self,
        path: &PathBuf,
        indexes: &Vec<TreeIndex>,
    ) -> Result<(), Error> {
        let vector = crate::zstd_decode_path(path)?;

        for (hash, (_index, node)) in crate::get_internal_terminal(&vector, indexes)? {
            self.nodes.insert(hash, node);
        }

        Ok(())
    }

    pub fn collect_and_return_from_merkle_blobs(
        &mut self,
        jobs: &Vec<(Hash, PathBuf)>,
        hashes: &HashSet<Hash>,
    ) -> Result<Vec<(Hash, NodeHashToIndex)>, Error> {
        let mut grouped_results = Vec::new();
        grouped_results.par_extend(jobs.into_par_iter().map(
            |(hash, path)| -> Result<(Hash, (NodeHashToDeltaReaderNode, NodeHashToIndex)), Error> {
                Ok((
                    *hash,
                    crate::collect_and_return_from_merkle_blob(path, hashes, |key| {
                        self.nodes.contains_key(key)
                    })?,
                ))
            },
        ));

        let mut results: Vec<(Hash, NodeHashToIndex)> = Vec::new();
        let mut seen_hashes: HashSet<Hash> = HashSet::new();
        for result in grouped_results {
            let (hash, (nodes, node_hash_to_index)) = result?;
            self.nodes.extend(nodes);
            let mut filtered = HashMap::new();
            for (hash, index) in node_hash_to_index {
                if seen_hashes.insert(hash) {
                    filtered.insert(hash, index);
                }
            }
            results.push((hash, filtered));
        }

        Ok(results)
    }

    pub fn collect_from_merkle_blobs(
        &mut self,
        jobs: &Vec<(PathBuf, Vec<TreeIndex>)>,
    ) -> Result<(), Error> {
        let mut results = Vec::new();

        results.par_extend(jobs.into_par_iter().map(
            |(path, indexes)| -> Result<HashMap<Hash, (TreeIndex, DeltaReaderNode)>, Error> {
                let vector = crate::zstd_decode_path(path)?;
                crate::get_internal_terminal(&vector, indexes)
            },
        ));

        for result in results {
            // admittedly just spitting out the first error here
            for (hash, (_index, node)) in result? {
                self.nodes.insert(hash, node);
            }
        }

        Ok(())
    }

    pub fn create_merkle_blob_and_filter_unused_nodes(
        &mut self,
        root_hash: Hash,
        interested_hashes: &HashSet<Hash>,
    ) -> Result<MerkleBlob, Error> {
        let mut all_used_hashes: HashSet<Hash> = HashSet::new();
        let merkle_blob = MerkleBlob::build_blob_from_node_list(
            &self.nodes,
            root_hash,
            interested_hashes,
            &mut all_used_hashes,
        )?;

        self.nodes.retain(|k, _v| all_used_hashes.contains(k));

        Ok(merkle_blob)
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl DeltaReader {
    #[new]
    pub fn py_init(internal_nodes: InternalNodesMap, leaf_nodes: LeafNodesMap) -> PyResult<Self> {
        Ok(Self::new(internal_nodes, leaf_nodes)?)
    }

    #[pyo3(name = "get_missing_hashes")]
    pub fn py_get_missing_hashes(&self, root_hash: Hash) -> PyResult<HashSet<Hash>> {
        Ok(self.get_missing_hashes(root_hash))
    }

    #[pyo3(name = "add_internal_nodes")]
    pub fn py_add_internal_nodes(&mut self, internal_nodes: InternalNodesMap) {
        for (hash, (left, right)) in internal_nodes {
            self.nodes
                .insert(hash, DeltaReaderNode::Internal { left, right });
        }
    }

    #[pyo3(name = "add_leaf_nodes")]
    pub fn py_add_leaf_nodes(&mut self, leaf_nodes: LeafNodesMap) {
        for (hash, (key, value)) in leaf_nodes {
            self.nodes
                .insert(hash, DeltaReaderNode::Leaf { key, value });
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    #[pyo3(name = "collect_from_merkle_blob")]
    pub fn py_collect_from_merkle_blob(
        &mut self,
        path: PathBuf,
        indexes: Vec<TreeIndex>,
    ) -> PyResult<()> {
        self.collect_from_merkle_blob(&path, &indexes)?;

        Ok(())
    }

    #[allow(clippy::needless_pass_by_value)]
    #[pyo3(name = "collect_and_return_from_merkle_blobs")]
    pub fn py_collect_and_return_from_merkle_blobs(
        &mut self,
        py: Python<'_>,
        jobs: Vec<(Hash, PathBuf)>,
        hashes: HashSet<Hash>,
    ) -> PyResult<Vec<(Hash, NodeHashToIndex)>> {
        let mut extracted_jobs: Vec<(Hash, PathBuf)> = Vec::new();
        for (hash, path) in jobs {
            extracted_jobs.push((hash, path));
        }

        Ok(py.detach(|| self.collect_and_return_from_merkle_blobs(&extracted_jobs, &hashes))?)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[pyo3(name = "collect_from_merkle_blobs")]
    pub fn py_collect_from_merkle_blobs(
        &mut self,
        py: Python<'_>,
        jobs: Vec<(PathBuf, Vec<TreeIndex>)>,
    ) -> PyResult<()> {
        let mut pathed_jobs: Vec<(PathBuf, Vec<TreeIndex>)> = Vec::new();
        for (path, indexes) in jobs {
            pathed_jobs.push((path, indexes));
        }
        py.detach(|| self.collect_from_merkle_blobs(&pathed_jobs))?;

        Ok(())
    }

    #[allow(clippy::needless_pass_by_value)]
    #[pyo3(name = "create_merkle_blob_and_filter_unused_nodes")]
    pub fn py_create_merkle_blob_and_filter_unused_nodes(
        &mut self,
        root_hash: Hash,
        interested_hashes: HashSet<Hash>,
    ) -> Result<MerkleBlob, Error> {
        self.create_merkle_blob_and_filter_unused_nodes(root_hash, &interested_hashes)
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl DeltaFileCache {
    #[allow(clippy::needless_pass_by_value)]
    #[new]
    fn py_new(path: PathBuf) -> PyResult<Self> {
        Ok(Self::new(&path)?)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[pyo3(name = "load_previous_hashes")]
    pub fn py_load_previous_hashes(&mut self, path: PathBuf) -> PyResult<()> {
        Ok(self.load_previous_hashes(&path)?)
    }

    #[pyo3(name = "get_index")]
    pub fn py_get_index(&self, hash: Hash) -> PyResult<TreeIndex> {
        Ok(self.get_index(hash)?)
    }

    #[pyo3(name = "seen_previous_hash")]
    pub fn py_seen_previous_hash(&self, hash: Hash) -> bool {
        self.seen_previous_hash(hash)
    }

    #[pyo3(name = "get_raw_node")]
    pub fn py_get_raw_node(&mut self, index: TreeIndex) -> PyResult<Node> {
        Ok(self.get_raw_node(index)?)
    }

    #[pyo3(name = "get_hash_at_index")]
    pub fn py_get_hash_at_index(&self, index: TreeIndex) -> PyResult<Option<Hash>> {
        Ok(self.get_hash_at_index(index)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::test_util::traversal_blob;
    use crate::merkle::test_util::{generate_hash, generate_kvid, HASH_ONE, HASH_TWO, HASH_ZERO};
    use crate::{InsertLocation, InternalNodesMap, LeafNodesMap};
    use expect_test::expect;
    use rstest::rstest;
    use std::iter::zip;
    use tempfile;

    fn incomplete_delta_reader() -> DeltaReader {
        let mut internal_nodes_map = InternalNodesMap::new();
        let mut leaf_nodes_map = LeafNodesMap::new();

        internal_nodes_map.insert(HASH_ZERO, (HASH_ONE, HASH_TWO));
        leaf_nodes_map.insert(HASH_ONE, (KeyId(0), ValueId(1)));

        DeltaReader::new(internal_nodes_map, leaf_nodes_map).unwrap()
    }

    fn complete_delta_reader() -> DeltaReader {
        let mut delta_reader = incomplete_delta_reader();
        delta_reader.nodes.insert(
            HASH_TWO,
            DeltaReaderNode::Leaf {
                key: KeyId(2),
                value: ValueId(3),
            },
        );

        delta_reader
    }

    #[test]
    fn test_root_hash_missing() {
        let internal_nodes_map = InternalNodesMap::new();
        let leaf_nodes_map = LeafNodesMap::new();
        let delta_reader = DeltaReader::new(internal_nodes_map, leaf_nodes_map).unwrap();
        let missing = delta_reader.get_missing_hashes(HASH_ZERO);
        let expected = expect![[r"
            {
                Hash(
                    0000000000000000000000000000000000000000000000000000000000000000,
                ),
            }
        "]];
        expected.assert_debug_eq(&missing);
    }

    #[test]
    fn test_delta_reader_get_missing_hashes_one_known_one_unknown() {
        let delta_reader = incomplete_delta_reader();
        let missing = delta_reader.get_missing_hashes(HASH_ZERO);

        let expected = expect![[r"
            {
                Hash(
                    0202020202020202020202020202020202020202020202020202020202020202,
                ),
            }
        "]];
        expected.assert_debug_eq(&missing);
    }

    #[test]
    fn test_delta_reader_collect_from_merkle_blob_completes_incomplete() {
        let mut delta_reader = incomplete_delta_reader();
        let mut merkle_blob = MerkleBlob::new(Vec::new()).unwrap();
        merkle_blob
            .insert(KeyId(2), ValueId(3), &HASH_TWO, InsertLocation::AsRoot {})
            .unwrap();

        let dir_path = tempfile::tempdir().unwrap();
        let leaf_blob_path = dir_path.path().join("merkle_blob");
        merkle_blob.to_path(&leaf_blob_path).unwrap();
        delta_reader
            .collect_from_merkle_blob(&leaf_blob_path, &vec![TreeIndex(0)])
            .unwrap();

        let missing = delta_reader.get_missing_hashes(HASH_ZERO);

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            {}
        "#]];
        expected.assert_debug_eq(&missing);
    }

    #[rstest]
    fn test_delta_reader_collect_from_merkle_blob_is_complete(traversal_blob: MerkleBlob) {
        let mut delta_reader = DeltaReader {
            nodes: HashMap::new(),
        };
        let dir_path = tempfile::tempdir().unwrap();
        let blob_path = dir_path.path().join("merkle_blob");
        traversal_blob.to_path(&blob_path).unwrap();
        delta_reader
            .collect_from_merkle_blob(&blob_path, &vec![TreeIndex(0)])
            .unwrap();

        let missing = delta_reader.get_missing_hashes(
            traversal_blob
                .get_hash_at_index(TreeIndex(0))
                .unwrap()
                .expect("Expected root hash"),
        );

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            {}
        "#]];
        expected.assert_debug_eq(&missing);
    }

    #[test]
    fn test_delta_reader_collect_from_merkle_blobs() {
        let mut delta_reader = incomplete_delta_reader();
        let mut merkle_blob = MerkleBlob::new(Vec::new()).unwrap();
        merkle_blob
            .insert(KeyId(2), ValueId(3), &HASH_TWO, InsertLocation::AsRoot {})
            .unwrap();

        let dir_path = tempfile::tempdir().unwrap();
        let leaf_blob_path = dir_path.path().join("merkle_blob");
        merkle_blob.to_path(&leaf_blob_path).unwrap();
        delta_reader
            .collect_from_merkle_blobs(&vec![(leaf_blob_path, vec![TreeIndex(0)])])
            .unwrap();

        let missing = delta_reader.get_missing_hashes(
            merkle_blob
                .get_hash_at_index(TreeIndex(0))
                .unwrap()
                .expect("Expected root hash"),
        );

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            {}
        "#]];
        expected.assert_debug_eq(&missing);
    }

    #[test]
    #[should_panic(expected = "integrity check failed while dropping merkle blob: CycleFound")]
    fn test_delta_reader_create_merkle_blob_incomplete_fails() {
        let mut delta_reader = incomplete_delta_reader();

        delta_reader
            .create_merkle_blob_and_filter_unused_nodes(HASH_ZERO, &HashSet::new())
            .expect_err("incomplete so should fail");
    }

    #[test]
    fn test_delta_reader_create_merkle_blob_works() {
        let mut delta_reader = complete_delta_reader();
        let interested_hashes: HashSet<Hash> = delta_reader.nodes.keys().copied().collect();

        let complete_blob = delta_reader
            .create_merkle_blob_and_filter_unused_nodes(HASH_ZERO, &interested_hashes)
            .unwrap();
        complete_blob.check_integrity().unwrap();
    }

    #[rstest]
    fn test_collect_and_return(traversal_blob: MerkleBlob) {
        let dir_path = tempfile::tempdir().unwrap();
        let file_path = dir_path.path().join("blob");
        traversal_blob.to_path(&file_path).unwrap();
        let hashes = traversal_blob
            .get_hashes_indexes(false)
            .unwrap()
            .into_keys()
            .collect::<HashSet<Hash>>();
        let root_hash = traversal_blob.get_hash(TreeIndex(0)).unwrap();

        let mut delta_reader = DeltaReader {
            nodes: HashMap::new(),
        };
        let mut root_hash_to_node_hash_to_index = delta_reader
            .collect_and_return_from_merkle_blobs(&vec![(root_hash, file_path)], &hashes)
            .unwrap();

        let (collected_root_hash, collected_node_hash_to_index) =
            root_hash_to_node_hash_to_index.pop().unwrap();
        assert_eq!(root_hash_to_node_hash_to_index.len(), 0);

        assert_eq!(collected_root_hash, root_hash);
        assert_eq!(
            collected_node_hash_to_index,
            traversal_blob.get_hashes_indexes(false).unwrap()
        );
    }

    #[rstest]
    fn test_delta_file_cache() {
        let num_inserts = 500;

        let mut merkle_blob = MerkleBlob::new(Vec::new()).unwrap();
        let mut kv_ids: Vec<(KeyId, ValueId)> = Vec::new();
        let mut hashes: Vec<Hash> = Vec::new();

        let mut previous_merkle_blob = MerkleBlob::new(Vec::new()).unwrap();
        let mut prev_kv_ids: Vec<(KeyId, ValueId)> = Vec::new();
        let mut prev_hashes: Vec<Hash> = Vec::new();

        for seed in 1..=num_inserts {
            let (key, value) = generate_kvid(seed);
            kv_ids.push((key, value));
            hashes.push(generate_hash(seed));

            let (key, value) = generate_kvid(num_inserts + seed);
            prev_kv_ids.push((key, value));
            prev_hashes.push(generate_hash(num_inserts + seed));
        }

        merkle_blob
            .batch_insert(zip(kv_ids, hashes.clone()).collect())
            .unwrap();
        merkle_blob.calculate_lazy_hashes().unwrap();

        previous_merkle_blob
            .batch_insert(zip(prev_kv_ids, prev_hashes.clone()).collect())
            .unwrap();
        previous_merkle_blob.calculate_lazy_hashes().unwrap();

        let dir_path = tempfile::tempdir().unwrap();
        let blob_path = dir_path.path().join("merkle_blob");
        merkle_blob.to_path(&blob_path).unwrap();
        let previous_blob_path = dir_path.path().join("previous_merkle_blob");
        previous_merkle_blob.to_path(&previous_blob_path).unwrap();

        let mut delta_cache_file = DeltaFileCache::new(&blob_path).unwrap();
        for hash in &hashes {
            let index = delta_cache_file.get_index(*hash).unwrap();
            let received_hash = delta_cache_file.get_hash_at_index(index).unwrap();
            assert_eq!(received_hash, Some(*hash));
            let node = delta_cache_file.get_raw_node(index).unwrap();
            assert_eq!(node.hash(), *hash);
        }

        delta_cache_file
            .load_previous_hashes(&previous_blob_path)
            .unwrap();
        for hash in &prev_hashes {
            let exists = delta_cache_file.seen_previous_hash(*hash);
            assert!(exists);
        }
    }
}
