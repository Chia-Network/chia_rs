#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};
#[cfg(feature = "py-bindings")]
use pyo3::{
    buffer::PyBuffer,
    pyclass, pymethods,
    types::{PyDict, PyDictMethods, PyListMethods, PyType},
    Bound, IntoPyObject, PyAny, PyResult, Python,
};

use crate::merkle::iterators::{BreadthFirstIterator, LeftChildFirstIterator, ParentFirstIterator};
use crate::merkle::{
    deltas, format, proof_of_inclusion,
    util::{sha256_bytes, sha256_num},
};
use crate::{
    merkle::error::Error, Block, BlockBytes, Hash, InternalNode, KeyId, LeafNode, Node,
    NodeMetadata, NodeType, Parent, TreeIndex, ValueId, BLOCK_SIZE,
};
use bitvec::prelude::BitVec;
use chia_protocol::Bytes32;
use chia_sha2::Sha256;
use chia_streamable_macro::Streamable;
#[cfg(feature = "py-bindings")]
use chia_traits::Streamable;
use indexmap::IndexSet;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
#[cfg(feature = "py-bindings")]
use std::iter::zip;
use std::ops::Range;
use std::path::PathBuf;

// assumptions
// - root is at index 0
// - any case with no keys will have a zero length blob

pub fn zstd_decode_path(path: &PathBuf) -> Result<Vec<u8>, Error> {
    let mut vector: Vec<u8> = Vec::new();
    let file = std::fs::File::open(path)?;
    let mut decoder = zstd::Decoder::new(file)?;
    decoder.read_to_end(&mut vector)?;

    Ok(vector)
}

pub fn internal_hash(left_hash: &Hash, right_hash: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(b"\x02");
    hasher.update(left_hash.0);
    hasher.update(right_hash.0);

    Hash(Bytes32::new(hasher.finalize()))
}

pub fn calculate_internal_hash(hash: &Hash, other_hash_side: Side, other_hash: &Hash) -> Hash {
    match other_hash_side {
        Side::Left => internal_hash(other_hash, hash),
        Side::Right => internal_hash(hash, other_hash),
    }
}

#[cfg_attr(feature = "py-bindings", derive(PyJsonDict, PyStreamable))]
#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Streamable)]
pub enum Side {
    Left = 0,
    Right = 1,
}

#[cfg_attr(feature = "py-bindings", pyclass)]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum InsertLocation {
    // error: Unit variant `Auto` is not yet supported in a complex enum
    // = help: change to a struct variant with no fields: `Auto { }`
    // = note: the enum is complex because of non-unit variant `Leaf`
    Auto {},
    AsRoot {},
    Leaf { index: TreeIndex, side: Side },
}

pub fn block_range(index: TreeIndex) -> Range<usize> {
    let block_start = index.0 as usize * BLOCK_SIZE;
    block_start..block_start + BLOCK_SIZE
}

#[cfg_attr(feature = "py-bindings", pyclass)]
#[derive(Clone, Debug)]
pub struct BlockStatusCache {
    free_indexes: IndexSet<TreeIndex>,
    key_to_index: HashMap<KeyId, TreeIndex>,
    leaf_hash_to_index: HashMap<Hash, TreeIndex>,
}

impl BlockStatusCache {
    fn new(blob: &[u8]) -> Result<Self, Error> {
        let index_count = blob.len() / BLOCK_SIZE;

        let mut seen_indexes: BitVec<u64, bitvec::order::Lsb0> = BitVec::repeat(false, index_count);
        let mut key_to_index: HashMap<KeyId, TreeIndex> = HashMap::default();
        let mut leaf_hash_to_index: HashMap<Hash, TreeIndex> = HashMap::default();

        for item in LeftChildFirstIterator::new(blob, None) {
            let (index, block) = item?;
            seen_indexes.set(index.0 as usize, true);

            if let Node::Leaf(leaf) = block.node {
                if key_to_index.insert(leaf.key, index).is_some() {
                    return Err(Error::KeyAlreadyPresent());
                }
                if leaf_hash_to_index.insert(leaf.hash, index).is_some() {
                    return Err(Error::HashAlreadyPresent());
                }
            }
        }

        let mut free_indexes: IndexSet<TreeIndex> = IndexSet::new();
        for (index, seen) in seen_indexes.iter().enumerate() {
            if !seen {
                free_indexes.insert(TreeIndex(index as u32));
            }
        }

        Ok(Self {
            free_indexes,
            key_to_index,
            leaf_hash_to_index,
        })
    }

    fn iter_keys_indexes(&self) -> impl Iterator<Item = (&KeyId, &TreeIndex)> {
        self.key_to_index.iter()
    }

    fn pop_free_index(&mut self) -> Option<TreeIndex> {
        let maybe_index = self.free_indexes.iter().next().copied();
        if let Some(index) = maybe_index {
            self.free_indexes.shift_remove(&index);
        }

        maybe_index
    }

    fn get_index_by_key(&self, key: KeyId) -> Option<&TreeIndex> {
        self.key_to_index.get(&key)
    }

    fn get_index_by_leaf_hash(&self, hash: &Hash) -> Option<&TreeIndex> {
        self.leaf_hash_to_index.get(hash)
    }

    #[must_use]
    fn is_index_free(&self, index: TreeIndex) -> bool {
        self.free_indexes.contains(&index)
    }

    fn leaf_count(&self) -> usize {
        self.key_to_index.len()
    }

    fn free_index_count(&self) -> usize {
        self.free_indexes.len()
    }

    fn no_keys(&self) -> bool {
        self.key_to_index.is_empty()
    }

    fn contains_key(&self, key: KeyId) -> bool {
        self.key_to_index.contains_key(&key)
    }

    fn contains_leaf_hash(&self, hash: &Hash) -> bool {
        self.leaf_hash_to_index.contains_key(hash)
    }

    fn clear(&mut self) {
        self.key_to_index.clear();
        self.free_indexes.clear();
        self.leaf_hash_to_index.clear();
    }

    fn add_internal(&mut self, index: TreeIndex) {
        self.free_indexes.shift_remove(&index);
    }

    fn add_leaf(&mut self, index: TreeIndex, leaf: LeafNode) {
        self.free_indexes.shift_remove(&index);

        self.key_to_index.insert(leaf.key, index);
        self.leaf_hash_to_index.insert(leaf.hash, index);
    }

    fn remove_internal(&mut self, index: TreeIndex) {
        self.free_indexes.insert(index);
    }

    fn remove_leaf(&mut self, node: &LeafNode) -> Result<(), Error> {
        let Some(index) = self.key_to_index.remove(&node.key) else {
            return Err(Error::UnknownKey(node.key));
        };
        self.leaf_hash_to_index.remove(&node.hash);

        self.free_indexes.insert(index);

        Ok(())
    }

    fn move_index(&mut self, source: TreeIndex, destination: TreeIndex) -> Result<(), Error> {
        // to be called _after_ having written to the destination index
        // TODO: not checking it is within bounds of the present blob
        if self.free_indexes.contains(&source) {
            return Err(Error::MoveSourceIndexNotInUse(source));
        }
        // TODO: not checking it is within bounds of the present blob
        if self.free_indexes.contains(&destination) {
            return Err(Error::MoveDestinationIndexNotInUse(destination));
        }

        self.free_indexes.insert(source);

        Ok(())
    }
}

pub type NodeHashToIndex = HashMap<Hash, TreeIndex>;
pub type NodeHashToDeltaReaderNode = HashMap<Hash, deltas::DeltaReaderNode>;

pub fn collect_and_return_from_merkle_blob(
    path: &PathBuf,
    hashes: &HashSet<Hash>,
    known: impl Fn(&Hash) -> bool,
) -> Result<(NodeHashToDeltaReaderNode, NodeHashToIndex), Error> {
    let mut nodes = NodeHashToDeltaReaderNode::new();
    let blob = zstd_decode_path(path)?;
    let mut node_hash_to_index = NodeHashToIndex::new();

    let mut index_to_hash: HashMap<TreeIndex, Hash> = HashMap::new();

    let mut in_subtree: HashSet<Hash> = HashSet::new();
    let mut index_stack: Vec<(TreeIndex, bool)> = Vec::new();
    index_stack.push((TreeIndex(0), false));
    loop {
        let Some((index, visited)) = index_stack.pop() else {
            break;
        };

        let block = format::try_get_block(&blob, index)?;

        let node_hash = block.node.hash();
        index_to_hash.insert(index, node_hash);
        if known(&node_hash) {
            continue;
        }

        match block.node {
            Node::Internal(InternalNode {
                hash, left, right, ..
            }) => {
                if visited {
                    node_hash_to_index.insert(hash, index);
                    if !in_subtree.is_empty() {
                        nodes.insert(
                            hash,
                            deltas::DeltaReaderNode::Internal {
                                left: *index_to_hash.get(&left).unwrap(),
                                right: *index_to_hash.get(&right).unwrap(),
                            },
                        );
                    }

                    in_subtree.remove(&hash);
                } else {
                    if hashes.contains(&hash) {
                        in_subtree.insert(hash);
                    }

                    index_stack.push((index, true));
                    index_stack.push((right, false));
                    index_stack.push((left, false));
                }
            }
            Node::Leaf(LeafNode {
                hash, key, value, ..
            }) => {
                if !in_subtree.is_empty() || hashes.contains(&hash) {
                    nodes.insert(hash, deltas::DeltaReaderNode::Leaf { key, value });
                }

                node_hash_to_index.insert(hash, index);
            }
        }
    }

    Ok((nodes, node_hash_to_index))
}

pub type InternalNodesMap = HashMap<Hash, (Hash, Hash)>;
pub type LeafNodesMap = HashMap<Hash, (KeyId, ValueId)>;

/// Stores a DataLayer merkle tree in bytes and provides serialization on each access so that only
/// the parts presently in use are stored in active objects.  The bytes are grouped as blocks of
/// equal size regardless of being internal vs. external nodes so that block indexes can be used
/// for references to particular nodes and readily converted to byte indexes.  The leaf nodes
/// do not hold the DataLayer key and value data but instead an id for each of the key and value
/// such that the code using a merkle blob can store the key and value as they see fit.  Each node
/// stores the hash for the merkle aspect of the tree.
#[cfg_attr(feature = "py-bindings", pyclass(get_all))]
#[derive(Clone, Debug)]
pub struct MerkleBlob {
    pub(crate) blob: Vec<u8>,
    block_status_cache: BlockStatusCache,
    // TODO: used by fuzzing, some cleaner way?  making it cfg-dependent is annoying with
    //       the type stubs
    pub check_integrity_on_drop: bool,
}

impl MerkleBlob {
    pub fn new(blob: Vec<u8>) -> Result<Self, Error> {
        let length = blob.len();
        let remainder = length % BLOCK_SIZE;
        if remainder != 0 {
            return Err(Error::InvalidBlobLength(remainder));
        }

        let block_status_cache = BlockStatusCache::new(&blob)?;

        let self_ = Self {
            blob,
            block_status_cache,
            check_integrity_on_drop: cfg!(test),
        };

        Ok(self_)
    }

    pub fn from_path(path: &PathBuf) -> Result<Self, Error> {
        let vector = zstd_decode_path(path)?;

        Self::new(vector)
    }

    pub fn to_path(&self, path: &PathBuf) -> Result<(), Error> {
        let directory = path.parent().ok_or(std::io::Error::new(
            std::io::ErrorKind::IsADirectory,
            format!(
                "path must be a file, root directory given: {}",
                path.display()
            ),
        ))?;
        std::fs::create_dir_all(directory)?;
        let file = std::fs::File::create(path)?;
        let mut encoder = zstd::Encoder::new(file, 0)?;
        encoder.write_all(&self.blob)?;
        encoder.finish()?;

        Ok(())
    }

    fn clear(&mut self) {
        self.blob.clear();
        self.block_status_cache.clear();
    }

    pub fn insert(
        &mut self,
        key: KeyId,
        value: ValueId,
        hash: &Hash,
        insert_location: InsertLocation,
    ) -> Result<TreeIndex, Error> {
        if self.block_status_cache.contains_key(key) {
            return Err(Error::KeyAlreadyPresent());
        }
        if self.block_status_cache.contains_leaf_hash(hash) {
            return Err(Error::HashAlreadyPresent());
        }

        let insert_location = match insert_location {
            InsertLocation::Auto {} => self.get_random_insert_location_by_key_id(key)?,
            _ => insert_location,
        };

        match insert_location {
            InsertLocation::Auto {} => {
                unreachable!("this should have been caught and processed above")
            }
            InsertLocation::AsRoot {} => {
                if !self.block_status_cache.no_keys() {
                    return Err(Error::UnableToInsertAsRootOfNonEmptyTree());
                }
                self.insert_first(key, value, hash)
            }
            InsertLocation::Leaf { index, side } => {
                let old_leaf = self.get_node(index)?.try_into_leaf()?;

                let internal_node_hash = match side {
                    Side::Left => internal_hash(hash, &old_leaf.hash),
                    Side::Right => internal_hash(&old_leaf.hash, hash),
                };

                let node = LeafNode {
                    parent: Parent(None),
                    hash: *hash,
                    key,
                    value,
                };

                if self.block_status_cache.leaf_count() == 1 {
                    self.insert_second(node, &old_leaf, &internal_node_hash, side)
                } else {
                    self.insert_third_or_later(node, &old_leaf, index, &internal_node_hash, side)
                }
            }
        }
    }

    fn insert_first(
        &mut self,
        key: KeyId,
        value: ValueId,
        hash: &Hash,
    ) -> Result<TreeIndex, Error> {
        let new_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node::Leaf(LeafNode {
                parent: Parent(None),
                key,
                value,
                hash: *hash,
            }),
        };

        let index = self.extend_index();
        self.insert_entry_to_blob(index, &new_leaf_block)?;

        Ok(index)
    }

    fn insert_second(
        &mut self,
        mut node: LeafNode,
        old_leaf: &LeafNode,
        internal_node_hash: &Hash,
        side: Side,
    ) -> Result<TreeIndex, Error> {
        self.clear();
        let root_index = self.get_new_index();
        let left_index = self.get_new_index();
        let right_index = self.get_new_index();

        let new_internal_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node::Internal(InternalNode {
                parent: Parent(None),
                left: left_index,
                right: right_index,
                hash: *internal_node_hash,
            }),
        };

        self.insert_entry_to_blob(root_index, &new_internal_block)?;

        node.parent = Parent(Some(TreeIndex(0)));

        let nodes = [
            (
                match side {
                    Side::Left => right_index,
                    Side::Right => left_index,
                },
                LeafNode {
                    parent: Parent(Some(TreeIndex(0))),
                    key: old_leaf.key,
                    value: old_leaf.value,
                    hash: old_leaf.hash,
                },
            ),
            (
                match side {
                    Side::Left => left_index,
                    Side::Right => right_index,
                },
                node,
            ),
        ];

        for (index, node) in nodes {
            let block = Block {
                metadata: NodeMetadata {
                    node_type: NodeType::Leaf,
                    dirty: false,
                },
                node: Node::Leaf(node),
            };

            self.insert_entry_to_blob(index, &block)?;
        }

        Ok(nodes[1].0)
    }

    fn insert_third_or_later(
        &mut self,
        mut node: LeafNode,
        old_leaf: &LeafNode,
        old_leaf_index: TreeIndex,
        internal_node_hash: &Hash,
        side: Side,
    ) -> Result<TreeIndex, Error> {
        let new_leaf_index = self.get_new_index();
        let new_internal_node_index = self.get_new_index();

        node.parent = Parent(Some(new_internal_node_index));

        let new_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node::Leaf(node),
        };
        self.insert_entry_to_blob(new_leaf_index, &new_leaf_block)?;

        let (left_index, right_index) = match side {
            Side::Left => (new_leaf_index, old_leaf_index),
            Side::Right => (old_leaf_index, new_leaf_index),
        };
        let new_internal_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node::Internal(InternalNode {
                parent: old_leaf.parent,
                left: left_index,
                right: right_index,
                hash: *internal_node_hash,
            }),
        };
        self.insert_entry_to_blob(new_internal_node_index, &new_internal_block)?;

        let old_parent_index = old_leaf.parent.0.expect("root found when not expected");

        self.update_parent(old_leaf_index, Some(new_internal_node_index))?;

        let mut old_parent_block = self.get_block(old_parent_index)?;
        if let Node::Internal(ref mut internal_node, ..) = old_parent_block.node {
            if old_leaf_index == internal_node.left {
                internal_node.left = new_internal_node_index;
            } else if old_leaf_index == internal_node.right {
                internal_node.right = new_internal_node_index;
            } else {
                panic!("child not a child of its parent");
            }
        } else {
            panic!("expected internal node but found leaf");
        }

        self.insert_entry_to_blob(old_parent_index, &old_parent_block)?;

        self.mark_lineage_as_dirty(old_parent_index)?;

        Ok(new_leaf_index)
    }

    pub fn batch_insert(
        &mut self,
        mut keys_values_hashes: Vec<((KeyId, ValueId), Hash)>,
    ) -> Result<(), Error> {
        // OPT: perhaps go back to taking an iterator?
        // OPT: would it be worthwhile to hold the entire blocks?
        let mut indexes = vec![];

        if self.block_status_cache.leaf_count() <= 1 {
            for _ in 0..2 {
                let Some(((key, value), hash)) = keys_values_hashes.pop() else {
                    return Ok(());
                };
                self.insert(key, value, &hash, InsertLocation::Auto {})?;
            }
        }

        for ((key, value), hash) in keys_values_hashes {
            let new_leaf_index = self.get_new_index();
            let new_block = Block {
                metadata: NodeMetadata {
                    node_type: NodeType::Leaf,
                    dirty: false,
                },
                node: Node::Leaf(LeafNode {
                    parent: Parent(None),
                    hash,
                    key,
                    value,
                }),
            };
            self.insert_entry_to_blob(new_leaf_index, &new_block)?;
            indexes.push(new_leaf_index);
        }

        // OPT: can we insert the top node first?  maybe more efficient to update it's children
        //      than to update the parents of the children when traversing leaf to sub-root?
        while indexes.len() > 1 {
            let mut new_indexes = vec![];

            for chunk in indexes.chunks(2) {
                let [index_1, index_2] = match chunk {
                    [index] => {
                        new_indexes.push(*index);
                        continue;
                    }
                    [index_1, index_2] => [*index_1, *index_2],
                    _ => unreachable!(
                        "chunk should always be either one or two long and be handled above"
                    ),
                };

                let new_internal_node_index = self.get_new_index();

                let mut hashes = vec![];
                for index in [index_1, index_2] {
                    let block = self.update_parent(index, Some(new_internal_node_index))?;
                    hashes.push(block.node.hash());
                }

                let new_block = Block {
                    metadata: NodeMetadata {
                        node_type: NodeType::Internal,
                        dirty: false,
                    },
                    node: Node::Internal(InternalNode {
                        parent: Parent(None),
                        hash: internal_hash(&hashes[0], &hashes[1]),
                        left: index_1,
                        right: index_2,
                    }),
                };

                self.insert_entry_to_blob(new_internal_node_index, &new_block)?;
                new_indexes.push(new_internal_node_index);
            }

            indexes = new_indexes;
        }

        if indexes.len() == 1 {
            // OPT: can we avoid this extra min height leaf traversal?
            let min_height_leaf = self.get_min_height_leaf()?;
            self.insert_subtree_at_key(min_height_leaf.key, indexes[0], Side::Left)?;
        }

        Ok(())
    }

    fn insert_subtree_at_key(
        &mut self,
        old_leaf_key: KeyId,
        new_index: TreeIndex,
        side: Side,
    ) -> Result<(), Error> {
        // TODO: seems like this ought to be fairly similar to regular insert

        struct Stuff {
            index: TreeIndex,
            hash: Hash,
        }

        let new_internal_node_index = self.get_new_index();
        let (old_leaf_index, old_leaf, _old_block) = self.get_leaf_by_key(old_leaf_key)?;
        let new_node = self.get_node(new_index)?;

        let new_stuff = Stuff {
            index: new_index,
            hash: new_node.hash(),
        };
        let old_stuff = Stuff {
            index: old_leaf_index,
            hash: old_leaf.hash,
        };
        let (left, right) = match side {
            Side::Left => (new_stuff, old_stuff),
            Side::Right => (old_stuff, new_stuff),
        };
        let internal_node_hash = internal_hash(&left.hash, &right.hash);

        let block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node::Internal(InternalNode {
                parent: old_leaf.parent,
                hash: internal_node_hash,
                left: left.index,
                right: right.index,
            }),
        };
        self.insert_entry_to_blob(new_internal_node_index, &block)?;
        self.update_parent(new_index, Some(new_internal_node_index))?;

        let Some(old_leaf_parent) = old_leaf.parent.0 else {
            return Err(Error::LeafCannotBeRootWhenInsertingSubtree());
        };

        let mut parent = self.get_block(old_leaf_parent)?;
        if let Node::Internal(ref mut internal) = parent.node {
            match old_leaf_index {
                x if x == internal.left => internal.left = new_internal_node_index,
                x if x == internal.right => internal.right = new_internal_node_index,
                _ => panic!("parent not a child a grandparent"),
            }
        } else {
            panic!("not handling this case now...")
        }
        self.insert_entry_to_blob(old_leaf_parent, &parent)?;
        self.mark_lineage_as_dirty(old_leaf_parent)?;
        self.update_parent(old_leaf_index, Some(new_internal_node_index))?;

        Ok(())
    }

    fn get_min_height_leaf(&self) -> Result<LeafNode, Error> {
        let (_index, block) = BreadthFirstIterator::new(&self.blob, None)
            .next()
            .ok_or(Error::UnableToFindALeaf())??;

        Ok(block
            .node
            .expect_leaf("unexpectedly found internal node first: <<self>>"))
    }

    pub fn delete(&mut self, key: KeyId) -> Result<(), Error> {
        let (leaf_index, leaf, _leaf_block) = self.get_leaf_by_key(key)?;
        self.block_status_cache.remove_leaf(&leaf)?;

        let Some(parent_index) = leaf.parent.0 else {
            self.clear();
            return Ok(());
        };

        let maybe_parent = self.get_node(parent_index)?;
        let Node::Internal(parent) = maybe_parent else {
            panic!("parent node not internal: {maybe_parent:?}")
        };
        let sibling_index = parent.sibling_index(leaf_index)?;
        let mut sibling_block = self.get_block(sibling_index)?;

        let Some(grandparent_index) = parent.parent.0 else {
            sibling_block.node.set_parent(Parent(None));
            let destination = TreeIndex(0);
            if let Node::Internal(node) = sibling_block.node {
                for child_index in [node.left, node.right] {
                    self.update_parent(child_index, Some(destination))?;
                }
            }

            self.insert_entry_to_blob(destination, &sibling_block)?;
            self.block_status_cache
                .move_index(sibling_index, destination)?;

            return Ok(());
        };

        self.block_status_cache.remove_internal(parent_index);
        let mut grandparent_block = self.get_block(grandparent_index)?;

        sibling_block
            .node
            .set_parent(Parent(Some(grandparent_index)));
        self.insert_entry_to_blob(sibling_index, &sibling_block)?;

        if let Node::Internal(ref mut internal) = grandparent_block.node {
            match parent_index {
                x if x == internal.left => internal.left = sibling_index,
                x if x == internal.right => internal.right = sibling_index,
                _ => panic!("parent not a child a grandparent"),
            }
        } else {
            panic!("grandparent not an internal node")
        }
        self.insert_entry_to_blob(grandparent_index, &grandparent_block)?;

        self.mark_lineage_as_dirty(grandparent_index)?;

        Ok(())
    }

    pub fn upsert(&mut self, key: KeyId, value: ValueId, new_hash: &Hash) -> Result<(), Error> {
        let Ok((leaf_index, mut leaf, mut block)) = self.get_leaf_by_key(key) else {
            self.insert(key, value, new_hash, InsertLocation::Auto {})?;
            return Ok(());
        };

        self.block_status_cache.remove_leaf(&leaf)?;
        leaf.hash.clone_from(new_hash);
        leaf.value = value;
        // OPT: maybe just edit in place?
        block.node = Node::Leaf(leaf);
        self.insert_entry_to_blob(leaf_index, &block)?;

        if let Some(parent) = block.node.parent().0 {
            self.mark_lineage_as_dirty(parent)?;
        }

        Ok(())
    }

    pub fn check_integrity(&self) -> Result<(), Error> {
        self.check_just_integrity()?;

        let mut clone = self.clone();
        clone.check_integrity_on_drop = false;
        clone.calculate_lazy_hashes()?;
        clone.check_just_integrity()
    }

    fn check_just_integrity(&self) -> Result<(), Error> {
        let mut leaf_count: usize = 0;
        let mut internal_count: usize = 0;
        let mut child_to_parent: HashMap<TreeIndex, TreeIndex> = HashMap::new();

        for item in ParentFirstIterator::new(&self.blob, None) {
            let (index, block) = item?;
            if let Some(parent) = block.node.parent().0 {
                if child_to_parent.remove(&index) != Some(parent) {
                    return Err(Error::IntegrityParentChildMismatch(index));
                }
            }
            match block.node {
                Node::Internal(node) => {
                    internal_count += 1;
                    child_to_parent.insert(node.left, index);
                    child_to_parent.insert(node.right, index);
                }
                Node::Leaf(node) => {
                    leaf_count += 1;
                    let cached_index = self
                        .block_status_cache
                        .get_index_by_key(node.key)
                        .ok_or(Error::IntegrityKeyNotInCache(node.key))?;
                    if *cached_index != index {
                        return Err(Error::IntegrityKeyToIndexCacheIndex(
                            node.key,
                            index,
                            *cached_index,
                        ));
                    }
                    assert!(
                        !self.block_status_cache.is_index_free(index),
                        "{}",
                        format!("active index found in free index list: {index:?}")
                    );
                }
            }
        }

        let key_to_index_cache_length = self.block_status_cache.key_to_index.len();
        if leaf_count != key_to_index_cache_length {
            return Err(Error::IntegrityKeyToIndexCacheLength(
                leaf_count,
                key_to_index_cache_length,
            ));
        }
        let leaf_hash_to_index_cache_length = self.block_status_cache.leaf_hash_to_index.len();
        if leaf_count != leaf_hash_to_index_cache_length {
            return Err(Error::IntegrityLeafHashToIndexCacheLength(
                leaf_count,
                leaf_hash_to_index_cache_length,
            ));
        }
        let total_count = leaf_count + internal_count + self.block_status_cache.free_index_count();
        let extend_index = self.extend_index();
        if total_count != extend_index.0 as usize {
            return Err(Error::IntegrityTotalNodeCount(extend_index, total_count));
        }
        if !child_to_parent.is_empty() {
            return Err(Error::IntegrityUnmatchedChildParentRelationships(
                child_to_parent.len(),
            ));
        }

        Ok(())
    }

    fn update_parent(
        &mut self,
        index: TreeIndex,
        parent: Option<TreeIndex>,
    ) -> Result<Block, Error> {
        let mut block = self.get_block(index)?;
        block.node.set_parent(Parent(parent));
        self.insert_entry_to_blob(index, &block)?;

        Ok(block)
    }

    fn mark_lineage_as_dirty(&mut self, index: TreeIndex) -> Result<(), Error> {
        let mut next_index = Some(index);

        while let Some(this_index) = next_index {
            let mut block = Block::from_bytes(self.get_block_bytes(this_index)?)?;

            if block.metadata.dirty {
                break;
            }

            block.metadata.dirty = true;
            self.insert_entry_to_blob(this_index, &block)?;
            next_index = block.node.parent().0;
        }

        Ok(())
    }

    fn get_new_index(&mut self) -> TreeIndex {
        match self.block_status_cache.pop_free_index() {
            None => {
                let index = self.extend_index();
                self.blob.extend_from_slice(&[0; BLOCK_SIZE]);
                // NOTE: explicitly not marking index as free since that would hazard two
                //       sequential calls to this function through this path to both return
                //       the same index
                index
            }
            Some(new_index) => new_index,
        }
    }

    fn get_random_insert_location_by_seed(
        &self,
        seed_bytes: &[u8],
    ) -> Result<InsertLocation, Error> {
        let mut seed_bytes = Vec::from(seed_bytes);

        if self.blob.is_empty() {
            return Ok(InsertLocation::AsRoot {});
        }

        // NOTE: zero means left here but right below
        let final_side = if (seed_bytes
            .first()
            .ok_or(Error::ZeroLengthSeedNotAllowed())?
            & (1 << 7))
            == 0
        {
            Side::Left
        } else {
            Side::Right
        };

        let mut next_index = TreeIndex(0);
        let mut node = self.get_node(next_index)?;

        seed_bytes.reverse();
        loop {
            for byte in &seed_bytes {
                for bit_index in 0..8 {
                    match node {
                        Node::Leaf { .. } => {
                            return Ok(InsertLocation::Leaf {
                                index: next_index,
                                side: final_side,
                            })
                        }
                        Node::Internal(internal) => {
                            let bit = byte & (1 << bit_index) != 0;
                            next_index = if bit { internal.right } else { internal.left };
                            node = self.get_node(next_index)?;
                        }
                    }
                }
            }

            seed_bytes = sha256_bytes(&seed_bytes).0.into();
        }
    }

    pub fn get_hash_at_index(&self, index: TreeIndex) -> Result<Option<Hash>, Error> {
        if self.block_status_cache.no_keys() {
            return Ok(None);
        }

        let block = self.get_block(index)?;
        if block.metadata.dirty {
            return Err(Error::Dirty(index));
        }

        Ok(Some(block.node.hash()))
    }

    fn get_random_insert_location_by_key_id(&self, seed: KeyId) -> Result<InsertLocation, Error> {
        let seed = sha256_num(&seed.0);

        self.get_random_insert_location_by_seed(&seed.0)
    }

    fn extend_index(&self) -> TreeIndex {
        let blob_length = self.blob.len();
        let index: TreeIndex = TreeIndex((blob_length / BLOCK_SIZE) as u32);
        let remainder = blob_length % BLOCK_SIZE;
        assert_eq!(remainder, 0, "blob length {blob_length:?} not a multiple of {BLOCK_SIZE:?}, remainder: {remainder:?}");

        index
    }

    fn insert_entry_to_blob(&mut self, index: TreeIndex, block: &Block) -> Result<(), Error> {
        let new_block_bytes = block.to_bytes()?;
        let extend_index = self.extend_index();
        match index.cmp(&extend_index) {
            Ordering::Greater => return Err(Error::BlockIndexOutOfBounds(index)),
            Ordering::Equal => self.blob.extend_from_slice(&new_block_bytes),
            Ordering::Less => {
                self.blob[block_range(index)].copy_from_slice(&new_block_bytes);
            }
        }

        match block.node {
            Node::Leaf(leaf) => self.block_status_cache.add_leaf(index, leaf),
            Node::Internal(..) => self.block_status_cache.add_internal(index),
        }

        Ok(())
    }

    fn get_block(&self, index: TreeIndex) -> Result<Block, Error> {
        Block::from_bytes(self.get_block_bytes(index)?)
    }

    pub(crate) fn get_hash(&self, index: TreeIndex) -> Result<Hash, Error> {
        Ok(self.get_block(index)?.node.hash())
    }

    fn get_block_bytes(&self, index: TreeIndex) -> Result<BlockBytes, Error> {
        Ok(self
            .blob
            .get(block_range(index))
            .ok_or(Error::BlockIndexOutOfBounds(index))?
            .try_into()
            .unwrap_or_else(|e| panic!("failed getting block {index}: {e}")))
    }

    pub fn get_node(&self, index: TreeIndex) -> Result<Node, Error> {
        Ok(self.get_block(index)?.node)
    }

    pub fn get_leaf_by_key(&self, key: KeyId) -> Result<(TreeIndex, LeafNode, Block), Error> {
        let index = *self
            .block_status_cache
            .get_index_by_key(key)
            .ok_or(Error::UnknownKey(key))?;
        let block = self.get_block(index)?;
        let leaf = block.node.expect_leaf(&format!(
            "expected leaf for index from key cache: {index} -> <<self>>"
        ));

        Ok((index, leaf, block))
    }

    pub fn get_parent_index(&self, index: TreeIndex) -> Result<Parent, Error> {
        Ok(self.get_block(index)?.node.parent())
    }

    pub fn get_lineage_blocks_with_indexes(
        &self,
        index: TreeIndex,
    ) -> Result<Vec<(TreeIndex, Block)>, Error> {
        let mut next_index = Some(index);
        let mut lineage = vec![];

        while let Some(this_index) = next_index {
            let block = self.get_block(this_index)?;
            next_index = block.node.parent().0;
            lineage.push((this_index, block));
        }

        Ok(lineage)
    }

    pub fn get_lineage_with_indexes(
        &self,
        index: TreeIndex,
    ) -> Result<Vec<(TreeIndex, Node)>, Error> {
        Ok(self
            .get_lineage_blocks_with_indexes(index)?
            .iter()
            .map(|(index, block)| (*index, block.node))
            .collect())
    }

    pub fn get_lineage_indexes(&self, index: TreeIndex) -> Result<Vec<TreeIndex>, Error> {
        Ok(self
            .get_lineage_blocks_with_indexes(index)?
            .iter()
            .map(|(index, _block)| *index)
            .collect())
    }

    // pub fn iter(&self) -> MerkleBlobLeftChildFirstIterator<'_> {
    //     <&Self as IntoIterator>::into_iter(self)
    // }

    pub fn calculate_lazy_hashes(&mut self) -> Result<(), Error> {
        // OPT: yeah, storing the whole set of blocks via collect is not great
        for item in LeftChildFirstIterator::new_with_block_predicate(
            &self.blob,
            None,
            Some(|block: &Block| block.metadata.dirty),
        )
        .collect::<Vec<_>>()
        {
            let (index, mut block) = item?;
            assert!(block.metadata.dirty);

            let Node::Internal(ref leaf) = block.node else {
                panic!("leaves should not be dirty")
            };
            // OPT: obviously inefficient to re-get/deserialize these blocks inside
            //      an iteration that's already doing that
            let left_hash = self.get_hash(leaf.left)?;
            let right_hash = self.get_hash(leaf.right)?;
            block.update_hash(&left_hash, &right_hash);
            self.insert_entry_to_blob(index, &block)?;
        }

        Ok(())
    }

    pub fn get_keys_values(&self) -> Result<HashMap<KeyId, ValueId>, Error> {
        let mut map = HashMap::new();
        for (key, index) in self.block_status_cache.iter_keys_indexes() {
            let node = self.get_node(*index)?;
            let leaf = node.expect_leaf(
                "key was just retrieved from the key to index mapping, must be a leaf",
            );
            map.insert(*key, leaf.value);
        }

        Ok(map)
    }

    pub fn get_key_index(&self, key: KeyId) -> Result<TreeIndex, Error> {
        self.block_status_cache
            .get_index_by_key(key)
            .copied()
            .ok_or(Error::UnknownKey(key))
    }

    pub fn get_proof_of_inclusion(
        &self,
        key: KeyId,
    ) -> Result<proof_of_inclusion::ProofOfInclusion, Error> {
        let mut index = *self
            .block_status_cache
            .get_index_by_key(key)
            .ok_or(Error::UnknownKey(key))?;

        let node = self
            .get_node(index)?
            .expect_leaf("key to index mapping should only have leaves");

        let parents = self.get_lineage_blocks_with_indexes(index)?;
        let mut layers: Vec<proof_of_inclusion::ProofOfInclusionLayer> = Vec::new();
        let mut parents_iter = parents.iter();
        // first in the lineage is the index itself, second is the first parent
        parents_iter.next();
        for (next_index, block) in parents_iter {
            if block.metadata.dirty {
                return Err(Error::Dirty(*next_index));
            }
            let parent = block
                .node
                .expect_internal("all nodes after the first should be internal");
            let sibling_index = parent.sibling_index(index)?;
            let sibling_block = self.get_block(sibling_index)?;
            let sibling = sibling_block.node;
            let layer = proof_of_inclusion::ProofOfInclusionLayer {
                other_hash_side: parent.get_sibling_side(index)?,
                other_hash: sibling.hash(),
                combined_hash: parent.hash,
            };
            layers.push(layer);
            index = *next_index;
        }

        Ok(proof_of_inclusion::ProofOfInclusion {
            node_hash: node.hash,
            layers,
        })
    }

    pub fn get_node_by_hash(&self, node_hash: Hash) -> Result<(KeyId, ValueId), Error> {
        let Some(index) = self.block_status_cache.get_index_by_leaf_hash(&node_hash) else {
            return Err(Error::LeafHashNotFound(node_hash));
        };

        let node = self
            .get_node(*index)?
            .expect_leaf("should only have leaves in the leaf hash to index cache");

        Ok((node.key, node.value))
    }

    pub fn get_hashes(&self) -> Result<HashSet<Hash>, Error> {
        let mut hashes = HashSet::<Hash>::new();

        if self.blob.is_empty() {
            return Ok(hashes);
        }

        for item in ParentFirstIterator::new(&self.blob, None) {
            let (_, block) = item?;
            hashes.insert(block.node.hash());
        }

        Ok(hashes)
    }

    pub fn get_hashes_indexes(&self, leafs_only: bool) -> Result<HashMap<Hash, TreeIndex>, Error> {
        let mut hash_to_index = HashMap::new();

        if self.blob.is_empty() {
            return Ok(hash_to_index);
        }

        for item in ParentFirstIterator::new(&self.blob, None) {
            let (index, block) = item?;

            if leafs_only && block.metadata.node_type != NodeType::Leaf {
                continue;
            }

            hash_to_index.insert(block.node.hash(), index);
        }

        Ok(hash_to_index)
    }

    pub fn build_blob_from_node_list(
        nodes: &NodeHashToDeltaReaderNode,
        node_hash: Hash,
        interested_hashes: &HashSet<Hash>,
        all_used_hashes: &mut HashSet<Hash>,
    ) -> Result<Self, Error> {
        let mut hashes_and_indexes: Vec<(Hash, TreeIndex)> = Vec::new();
        let mut merkle_blob = Self::new(Vec::new())?;
        merkle_blob.inner_build_blob_from_node_list(
            nodes,
            node_hash,
            interested_hashes,
            &mut hashes_and_indexes,
            all_used_hashes,
        )?;

        Ok(merkle_blob)
    }

    fn inner_build_blob_from_node_list(
        &mut self,
        nodes: &NodeHashToDeltaReaderNode,
        node_hash: Hash,
        interested_hashes: &HashSet<Hash>,
        hashes_and_indexes: &mut Vec<(Hash, TreeIndex)>,
        all_used_hashes: &mut HashSet<Hash>,
    ) -> Result<TreeIndex, Error> {
        match nodes.get(&node_hash) {
            None => Err(Error::NodeHashNotInNodeMaps(node_hash)),
            Some(deltas::DeltaReaderNode::Leaf { key, value }) => {
                let index = self.get_new_index();
                self.insert_entry_to_blob(
                    index,
                    &Block {
                        metadata: NodeMetadata {
                            node_type: NodeType::Leaf,
                            dirty: false,
                        },
                        node: Node::Leaf(LeafNode {
                            hash: node_hash,
                            parent: Parent(None),
                            key: *key,
                            value: *value,
                        }),
                    },
                )?;

                if interested_hashes.contains(&node_hash) {
                    hashes_and_indexes.push((node_hash, index));
                }
                all_used_hashes.insert(node_hash);

                Ok(index)
            }
            Some(deltas::DeltaReaderNode::Internal { left, right }) => {
                let index = self.get_new_index();

                let left_index = self.inner_build_blob_from_node_list(
                    nodes,
                    *left,
                    interested_hashes,
                    hashes_and_indexes,
                    all_used_hashes,
                )?;
                let right_index = self.inner_build_blob_from_node_list(
                    nodes,
                    *right,
                    interested_hashes,
                    hashes_and_indexes,
                    all_used_hashes,
                )?;

                for child_index in [left_index, right_index] {
                    self.update_parent(child_index, Some(index))?;
                }
                let block = Block {
                    metadata: NodeMetadata {
                        node_type: NodeType::Internal,
                        dirty: false,
                    },
                    node: Node::Internal(InternalNode {
                        hash: node_hash,
                        parent: Parent(None),
                        left: left_index,
                        right: right_index,
                    }),
                };
                self.insert_entry_to_blob(index, &block)?;

                if interested_hashes.contains(&node_hash) {
                    hashes_and_indexes.push((node_hash, index));
                }
                all_used_hashes.insert(node_hash);

                Ok(index)
            }
        }
    }

    pub fn read_blob(&self) -> &Vec<u8> {
        &self.blob
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl MerkleBlob {
    #[allow(clippy::needless_pass_by_value)]
    #[new]
    pub fn py_init(blob: PyBuffer<u8>) -> PyResult<Self> {
        assert!(
            blob.is_c_contiguous(),
            "from_bytes() must be called with a contiguous buffer"
        );
        #[allow(unsafe_code)]
        let slice =
            unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };

        Ok(Self::new(Vec::from(slice))?)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[classmethod]
    #[pyo3(name = "from_path")]
    pub fn py_from_path(_cls: &Bound<'_, PyType>, path: PathBuf) -> PyResult<Self> {
        Ok(Self::from_path(&path)?)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[pyo3(name = "to_path")]
    pub fn py_to_path(&self, path: PathBuf) -> PyResult<()> {
        Ok(self.to_path(&path)?)
    }

    // it is known that memo is unused here, but is part of the interface of deepcopy
    #[allow(unused_variables)]
    #[must_use]
    #[pyo3(name = "__deepcopy__")]
    pub fn py_deepcopy(&self, memo: &pyo3::Bound<'_, pyo3::PyAny>) -> Self {
        self.clone()
    }

    #[pyo3(name = "insert", signature = (key, value, hash, reference_kid = None, side = None))]
    pub fn py_insert(
        &mut self,
        key: KeyId,
        value: ValueId,
        hash: Hash,
        reference_kid: Option<KeyId>,
        // TODO: should be a Side, but python has a different Side right now
        side: Option<u8>,
    ) -> PyResult<()> {
        let insert_location = match (reference_kid, side) {
            (None, None) => InsertLocation::Auto {},
            (Some(key), Some(side)) => InsertLocation::Leaf {
                index: *self
                    .block_status_cache
                    .get_index_by_key(key)
                    .ok_or(Error::UnknownKey(key))?,
                side: Side::from_bytes(&[side])?,
            },
            _ => Err(Error::IncompleteInsertLocationParameters())?,
        };
        self.insert(key, value, &hash, insert_location)?;

        Ok(())
    }

    #[pyo3(name = "upsert")]
    pub fn py_upsert(&mut self, key: KeyId, value: ValueId, new_hash: Hash) -> PyResult<()> {
        self.upsert(key, value, &new_hash)?;

        Ok(())
    }

    #[pyo3(name = "delete")]
    pub fn py_delete(&mut self, key: KeyId) -> PyResult<()> {
        Ok(self.delete(key)?)
    }

    #[pyo3(name = "get_raw_node")]
    pub fn py_get_raw_node(&mut self, index: TreeIndex) -> PyResult<Node> {
        Ok(self.get_node(index)?)
    }

    #[pyo3(name = "calculate_lazy_hashes")]
    pub fn py_calculate_lazy_hashes(&mut self) -> PyResult<()> {
        Ok(self.calculate_lazy_hashes()?)
    }

    #[pyo3(name = "get_lineage_with_indexes")]
    pub fn py_get_lineage_with_indexes<'py>(
        &self,
        index: TreeIndex,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let list = pyo3::types::PyList::empty(py);

        for (index, node) in self.get_lineage_with_indexes(index)? {
            list.append((index.into_pyobject(py)?, node.into_pyobject(py)?))?;
        }

        Ok(list.into_any())
    }

    #[pyo3(name = "get_nodes_with_indexes", signature = (index=None))]
    pub fn py_get_nodes_with_indexes<'py>(
        &self,
        index: Option<TreeIndex>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let list = pyo3::types::PyList::empty(py);

        for item in ParentFirstIterator::new(&self.blob, index) {
            let (index, block) = item?;
            list.append((index.into_pyobject(py)?, block.node.into_pyobject(py)?))?;
        }

        Ok(list.into_any())
    }

    #[pyo3(name = "empty")]
    pub fn py_empty(&self) -> PyResult<bool> {
        Ok(self.block_status_cache.no_keys())
    }

    #[pyo3(name = "get_root_hash")]
    pub fn py_get_root_hash(&self) -> PyResult<Option<Hash>> {
        self.py_get_hash_at_index(TreeIndex(0))
    }

    #[pyo3(name = "get_hash_at_index")]
    pub fn py_get_hash_at_index(&self, index: TreeIndex) -> PyResult<Option<Hash>> {
        Ok(self.get_hash_at_index(index)?)
    }

    #[pyo3(name = "batch_insert")]
    pub fn py_batch_insert(
        &mut self,
        keys_values: Vec<(KeyId, ValueId)>,
        hashes: Vec<Hash>,
    ) -> PyResult<()> {
        if keys_values.len() != hashes.len() {
            Err(Error::UnmatchedKeysAndValues(
                keys_values.len(),
                hashes.len(),
            ))?;
        }

        self.batch_insert(zip(keys_values, hashes).collect())?;

        Ok(())
    }

    #[pyo3(name = "__len__")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.blob.len())
    }

    #[pyo3(name = "get_keys_values")]
    pub fn py_get_keys_values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let map = self.get_keys_values()?;
        let dict = PyDict::new(py);
        for (key, value) in map {
            dict.set_item(key, value)?;
        }

        Ok(dict.into_any())
    }

    #[pyo3(name = "get_key_index")]
    pub fn py_get_key_index(&self, key: KeyId) -> PyResult<TreeIndex> {
        Ok(self.get_key_index(key)?)
    }

    #[pyo3(name = "get_proof_of_inclusion")]
    pub fn py_get_proof_of_inclusion(
        &self,
        key: KeyId,
    ) -> PyResult<proof_of_inclusion::ProofOfInclusion> {
        Ok(self.get_proof_of_inclusion(key)?)
    }

    #[pyo3(name = "get_node_by_hash")]
    pub fn py_get_node_by_hash(&self, node_hash: Hash) -> PyResult<(KeyId, ValueId)> {
        Ok(self.get_node_by_hash(node_hash)?)
    }

    #[pyo3(name = "get_hashes_indexes", signature = (leafs_only=false))]
    pub fn py_get_hashes_indexes(&self, leafs_only: bool) -> PyResult<HashMap<Hash, TreeIndex>> {
        Ok(self.get_hashes_indexes(leafs_only)?)
    }

    #[pyo3(name = "get_random_leaf_node")]
    pub fn py_get_random_leaf_node(&self, seed: &[u8]) -> PyResult<LeafNode> {
        let insert_location = self.get_random_insert_location_by_seed(seed)?;
        let InsertLocation::Leaf { index, side: _ } = insert_location else {
            Err(Error::UnableToFindALeaf())?
        };

        Ok(self.get_node(index)?.expect_leaf("matched leaf above"))
    }

    #[pyo3(name = "check_integrity")]
    pub fn py_check_integrity(&mut self) -> PyResult<()> {
        Ok(self.check_integrity()?)
    }
}

pub fn get_internal_terminal(
    blob: &[u8],
    indexes: &Vec<TreeIndex>,
) -> Result<HashMap<Hash, (TreeIndex, deltas::DeltaReaderNode)>, Error> {
    let mut nodes: HashMap<Hash, (TreeIndex, deltas::DeltaReaderNode)> = HashMap::new();
    let mut index_to_hash: HashMap<TreeIndex, Hash> = HashMap::new();

    for subroot_index in indexes {
        for item in LeftChildFirstIterator::new(blob, Some(*subroot_index)) {
            let (index, block) = item?;
            match block.node {
                Node::Internal(node) => {
                    index_to_hash.insert(index, node.hash);
                    nodes.insert(
                        node.hash,
                        (
                            index,
                            deltas::DeltaReaderNode::Internal {
                                left: *index_to_hash.get(&node.left).unwrap(),
                                right: *index_to_hash.get(&node.right).unwrap(),
                            },
                        ),
                    );
                }
                Node::Leaf(node) => {
                    index_to_hash.insert(index, node.hash);
                    nodes.insert(
                        node.hash,
                        (
                            index,
                            deltas::DeltaReaderNode::Leaf {
                                key: node.key,
                                value: node.value,
                            },
                        ),
                    );
                }
            }
        }
    }

    Ok(nodes)
}

#[cfg(any(test, debug_assertions))]
impl Drop for MerkleBlob {
    fn drop(&mut self) {
        if self.check_integrity_on_drop {
            self.check_integrity()
                .expect("integrity check failed while dropping merkle blob");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::test_util::{
        generate_hash, open_dot, small_blob, traversal_blob, HASH_ONE, HASH_ZERO,
    };
    use crate::merkle::util::sha256_num;
    use chia_traits::Streamable;
    use expect_test::expect;
    use rstest::rstest;
    use std::iter::zip;
    use std::time::{Duration, Instant};

    fn blob_tree_equality(this: &MerkleBlob, that: &MerkleBlob) -> bool {
        // NOTE: this is checking tree structure equality, not serialized bytes equality
        for item in zip(
            LeftChildFirstIterator::new(&this.blob, None),
            LeftChildFirstIterator::new(&that.blob, None),
        ) {
            let (Ok((_, self_block)), Ok((_, other_block))) = item else {
                return false;
            };
            if (self_block.metadata.dirty || other_block.metadata.dirty)
                || self_block.node.hash() != other_block.node.hash()
            {
                return false;
            }
            match self_block.node {
                // NOTE: this is effectively checked by the controlled overall traversal
                Node::Internal(..) => {}
                Node::Leaf(..) => return self_block.node == other_block.node,
            }
        }

        true
    }

    #[test]
    fn test_node_type_serialized_values() {
        assert_eq!(NodeType::Internal as u8, 0);
        assert_eq!(NodeType::Leaf as u8, 1);

        for node_type in [NodeType::Internal, NodeType::Leaf] {
            assert_eq!(
                Streamable::to_bytes(&node_type).unwrap()[0],
                node_type as u8,
            );
            assert_eq!(
                format::streamable_from_bytes_ignore_extra_bytes::<NodeType>(&[node_type as u8])
                    .unwrap(),
                node_type,
            );
        }
    }

    #[test]
    fn test_internal_hash() {
        // in Python: Program.to((left_hash, right_hash)).get_tree_hash_precalc(left_hash, right_hash)

        let left = Hash((0u8..32).collect::<Vec<_>>().try_into().unwrap());
        let right = Hash((32u8..64).collect::<Vec<_>>().try_into().unwrap());

        assert_eq!(
            internal_hash(&left, &right),
            Hash(Bytes32::new(
                clvm_utils::tree_hash_pair(
                    clvm_utils::TreeHash::new(left.0.to_bytes()),
                    clvm_utils::TreeHash::new(right.0.to_bytes()),
                )
                .to_bytes()
            )),
        );
    }

    #[rstest]
    fn test_node_metadata_from_to(
        #[values(false, true)] dirty: bool,
        #[values(NodeType::Internal, NodeType::Leaf)] node_type: NodeType,
    ) {
        let bytes: [u8; 2] = [Streamable::to_bytes(&node_type).unwrap()[0], dirty as u8];
        let object = NodeMetadata::from_bytes(&bytes).unwrap();
        assert_eq!(object, NodeMetadata { node_type, dirty },);
        assert_eq!(object.to_bytes().unwrap(), bytes);
    }

    #[rstest]
    fn test_get_lineage(small_blob: MerkleBlob) {
        let lineage = small_blob.get_lineage_with_indexes(TreeIndex(2)).unwrap();
        for (_, node) in &lineage {
            println!("{node:?}");
        }
        assert_eq!(lineage.len(), 2);
        let (_, last_node) = lineage.last().unwrap();
        assert_eq!(last_node.parent(), Parent(None));
    }

    #[rstest]
    #[case::right(0, TreeIndex(1), Side::Left)]
    #[case::left(0xff, TreeIndex(2), Side::Right)]
    fn test_get_random_insert_location_by_seed(
        #[case] seed: u8,
        #[case] expected_index: TreeIndex,
        #[case] expected_side: Side,
        small_blob: MerkleBlob,
    ) {
        let location = small_blob
            .get_random_insert_location_by_seed(&[seed; 32])
            .unwrap();

        assert_eq!(
            location,
            InsertLocation::Leaf {
                index: expected_index,
                side: expected_side
            },
        );
    }

    #[test]
    fn test_get_random_insert_location_by_seed_with_seed_too_short() {
        let mut blob = MerkleBlob::new(vec![]).unwrap();
        let seed = [0xff];
        let layer_count = 8 * seed.len() + 10;

        for n in 0..layer_count {
            let n = (n + 100) as i64;
            let key = KeyId(n);
            let value = ValueId(n);
            let hash = sha256_num(&key.0);
            let insert_location = blob.get_random_insert_location_by_seed(&seed).unwrap();
            blob.insert(key, value, &hash, insert_location).unwrap();
        }

        let location = blob.get_random_insert_location_by_seed(&seed).unwrap();

        let InsertLocation::Leaf { index, .. } = location else {
            panic!()
        };
        let lineage = blob.get_lineage_indexes(index).unwrap();

        assert_eq!(lineage.len(), layer_count);
        assert!(lineage.len() > seed.len() * 8);
    }

    #[rstest]
    fn test_just_insert_a_bunch(
        // just allowing parallelism of testing 100,000 inserts total
        #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9)] n: i64,
    ) {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let mut total_time = Duration::new(0, 0);

        let count = 10_000;
        let m = count * n;
        for i in m..(m + count) {
            let start = Instant::now();
            merkle_blob
                // NOTE: yeah this hash is garbage
                .insert(
                    KeyId(i),
                    ValueId(i),
                    &sha256_num(&i),
                    InsertLocation::Auto {},
                )
                .unwrap();
            let end = Instant::now();
            total_time += end.duration_since(start);
        }

        println!("total time: {total_time:?}");

        merkle_blob.calculate_lazy_hashes().unwrap();
    }

    #[test]
    fn test_delete_in_reverse_creates_matching_trees() {
        const COUNT: usize = 10;
        let mut dots = vec![];

        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();
        let mut reference_blobs = vec![];

        let key_value_ids: [i64; COUNT] = core::array::from_fn(|i| i as i64);

        for key_value_id in key_value_ids {
            let hash: Hash = sha256_num(&key_value_id);

            println!("inserting: {key_value_id}");
            merkle_blob.calculate_lazy_hashes().unwrap();
            reference_blobs.push(MerkleBlob::new(merkle_blob.blob.clone()).unwrap());
            merkle_blob
                .insert(
                    KeyId(key_value_id),
                    ValueId(key_value_id),
                    &hash,
                    InsertLocation::Auto {},
                )
                .unwrap();
            dots.push(merkle_blob.to_dot().unwrap().dump());
        }

        merkle_blob.check_integrity().unwrap();

        for key_value_id in key_value_ids.iter().rev() {
            println!("deleting: {key_value_id}");
            merkle_blob.delete(KeyId(*key_value_id)).unwrap();
            merkle_blob.calculate_lazy_hashes().unwrap();
            assert!(blob_tree_equality(
                &merkle_blob,
                &reference_blobs[*key_value_id as usize]
            ));
            dots.push(merkle_blob.to_dot().unwrap().dump());
        }
    }

    #[test]
    fn test_insert_first() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_id = 1;
        open_dot(merkle_blob.to_dot().unwrap().set_note("empty"));
        merkle_blob
            .insert(
                KeyId(key_value_id),
                ValueId(key_value_id),
                &sha256_num(&key_value_id),
                InsertLocation::Auto {},
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));

        assert_eq!(merkle_blob.block_status_cache.leaf_count(), 1);
    }

    #[rstest]
    fn test_insert_choosing_side(
        #[values(Side::Left, Side::Right)] side: Side,
        #[values(1, 2)] pre_count: usize,
    ) {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let mut last_key: KeyId = KeyId(0);
        for i in 1..=pre_count {
            let key_value = i as i64;
            open_dot(merkle_blob.to_dot().unwrap().set_note("empty"));
            merkle_blob
                .insert(
                    KeyId(key_value),
                    ValueId(key_value),
                    &sha256_num(&key_value),
                    InsertLocation::Auto {},
                )
                .unwrap();
            last_key = KeyId(key_value);
        }

        let key_value_id = (pre_count + 1) as i64;
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));
        merkle_blob
            .insert(
                KeyId(key_value_id),
                ValueId(key_value_id),
                &sha256_num(&key_value_id),
                InsertLocation::Leaf {
                    index: *merkle_blob
                        .block_status_cache
                        .get_index_by_key(last_key)
                        .unwrap(),
                    side,
                },
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));

        let sibling = merkle_blob
            .get_node(
                *merkle_blob
                    .block_status_cache
                    .get_index_by_key(last_key)
                    .unwrap(),
            )
            .unwrap();
        let parent = merkle_blob.get_node(sibling.parent().0.unwrap()).unwrap();
        let Node::Internal(internal) = parent else {
            panic!()
        };

        let left = merkle_blob
            .get_node(internal.left)
            .unwrap()
            .expect_leaf("<<self>>");
        let right = merkle_blob
            .get_node(internal.right)
            .unwrap()
            .expect_leaf("<<self>>");

        let expected_keys: [KeyId; 2] = match side {
            Side::Left => [KeyId(pre_count as i64 + 1), KeyId(pre_count as i64)],
            Side::Right => [KeyId(pre_count as i64), KeyId(pre_count as i64 + 1)],
        };
        assert_eq!([left.key, right.key], expected_keys);
    }

    #[test]
    fn test_delete_last() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_id = 1;
        open_dot(merkle_blob.to_dot().unwrap().set_note("empty"));
        merkle_blob
            .insert(
                KeyId(key_value_id),
                ValueId(key_value_id),
                &sha256_num(&key_value_id),
                InsertLocation::Auto {},
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));
        merkle_blob.check_integrity().unwrap();

        merkle_blob.delete(KeyId(key_value_id)).unwrap();

        assert_eq!(merkle_blob.block_status_cache.leaf_count(), 0);
    }

    #[rstest]
    fn test_delete_frees_index(mut small_blob: MerkleBlob) {
        let key = KeyId(0x0001_0203_0405_0607);
        let index = *small_blob.block_status_cache.get_index_by_key(key).unwrap();
        small_blob.delete(key).unwrap();

        assert_eq!(
            small_blob.block_status_cache.free_indexes,
            IndexSet::from([index, TreeIndex(1)])
        );
    }

    #[rstest]
    fn test_delete_with_internal_sibling(mut small_blob: MerkleBlob) {
        let key_to_delete = KeyId(0x0001_0203_0405_0607);
        let (other_key_index, _, _) = small_blob.get_leaf_by_key(key_to_delete).unwrap();

        small_blob
            .insert(
                KeyId(0x4041_4243_4445_4647),
                ValueId(0x5051_5253_5455_5657),
                &sha256_num(&0x4050),
                InsertLocation::Leaf {
                    index: other_key_index,
                    side: Side::Left,
                },
            )
            .unwrap();

        small_blob.delete(key_to_delete).unwrap();

        let keys_values = small_blob.get_keys_values().unwrap();
        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            [
                (
                    KeyId(
                        2315169217770759719,
                    ),
                    ValueId(
                        3472611983179986487,
                    ),
                ),
                (
                    KeyId(
                        4630054748589213255,
                    ),
                    ValueId(
                        5787497513998440023,
                    ),
                ),
            ]
        "#]];
        let mut keys_values = keys_values.iter().collect::<Vec<_>>();
        keys_values.sort();
        expected.assert_debug_eq(&keys_values);
    }

    #[rstest]
    fn test_get_new_index_with_free_index(mut small_blob: MerkleBlob) {
        open_dot(small_blob.to_dot().unwrap().set_note("initial"));
        let key = KeyId(0x0001_0203_0405_0607);
        let _ = small_blob.block_status_cache.get_index_by_key(key).unwrap();
        small_blob.delete(key).unwrap();
        open_dot(small_blob.to_dot().unwrap().set_note("after delete"));

        let expected = IndexSet::from([TreeIndex(1), TreeIndex(2)]);
        assert_eq!(small_blob.block_status_cache.free_indexes, expected);
    }

    #[rstest]
    fn test_dump_small_blob_bytes(small_blob: MerkleBlob) {
        println!("{}", hex::encode(small_blob.blob.clone()));
    }

    #[test]
    fn test_node_type_from_u8_invalid() {
        let invalid_value = 2;
        let actual =
            format::streamable_from_bytes_ignore_extra_bytes::<NodeType>(&[invalid_value as u8]);
        actual.expect_err("invalid node type value should fail");
    }

    #[test]
    fn test_node_specific_sibling_index_panics_for_unknown_sibling() {
        let node = InternalNode {
            parent: Parent(None),
            hash: sha256_num(&0),
            left: TreeIndex(0),
            right: TreeIndex(1),
        };
        let index = TreeIndex(2);
        node.sibling_index(TreeIndex(2))
            .expect_err(&Error::IndexIsNotAChild(index).to_string());
    }

    #[rstest]
    fn test_get_free_indexes(small_blob: MerkleBlob) {
        let mut blob = small_blob.blob.clone();
        let expected_free_index = TreeIndex((blob.len() / BLOCK_SIZE) as u32);
        blob.extend_from_slice(&[0; BLOCK_SIZE]);
        let block_status_cache = BlockStatusCache::new(&blob).unwrap();
        assert_eq!(
            block_status_cache.free_indexes,
            IndexSet::from([expected_free_index])
        );
    }

    #[test]
    fn test_merkle_blob_new_errs_for_nonmultiple_of_block_length() {
        MerkleBlob::new(vec![1]).expect_err("invalid length should fail");
    }

    #[rstest]
    fn test_upsert_inserts(small_blob: MerkleBlob) {
        let key = KeyId(1234);
        assert!(!small_blob.block_status_cache.contains_key(key));
        let value = ValueId(5678);

        let mut insert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        insert_blob
            .insert(key, value, &sha256_num(&key.0), InsertLocation::Auto {})
            .unwrap();
        open_dot(insert_blob.to_dot().unwrap().set_note("first after"));

        let mut upsert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        upsert_blob.upsert(key, value, &sha256_num(&key.0)).unwrap();
        open_dot(upsert_blob.to_dot().unwrap().set_note("first after"));

        assert_eq!(insert_blob.blob, upsert_blob.blob);
    }

    #[rstest]
    fn test_upsert_upserts(mut small_blob: MerkleBlob) {
        let before_blocks = LeftChildFirstIterator::new(&small_blob.blob, None).collect::<Vec<_>>();
        let (key, index) = small_blob
            .block_status_cache
            .iter_keys_indexes()
            .next()
            .unwrap();
        let original = small_blob.get_node(*index).unwrap().expect_leaf("<<self>>");
        let new_value = ValueId(original.value.0 + 1);

        small_blob.upsert(*key, new_value, &original.hash).unwrap();

        let after_blocks = LeftChildFirstIterator::new(&small_blob.blob, None).collect::<Vec<_>>();

        assert_eq!(before_blocks.len(), after_blocks.len());
        for item in zip(before_blocks, after_blocks) {
            let ((before_index, before_block), (after_index, after_block)) =
                (item.0.unwrap(), item.1.unwrap());
            assert_eq!(before_block.node.parent(), after_block.node.parent());
            assert_eq!(before_index, after_index);
            let before: LeafNode = match before_block.node {
                Node::Leaf(leaf) => leaf,
                Node::Internal(internal) => {
                    let Node::Internal(after) = after_block.node else {
                        panic!()
                    };
                    assert_eq!(internal.left, after.left);
                    assert_eq!(internal.right, after.right);
                    continue;
                }
            };
            let Node::Leaf(after) = after_block.node else {
                panic!()
            };
            assert_eq!(before.key, after.key);
            if before.key == original.key {
                assert_eq!(after.value, new_value);
            } else {
                assert_eq!(before.value, after.value);
            }
        }
    }

    #[test]
    fn test_double_insert_fails() {
        let mut blob = MerkleBlob::new(vec![]).unwrap();
        let kv = 0;
        blob.insert(
            KeyId(kv),
            ValueId(kv),
            &Hash(Bytes32::new([0u8; 32])),
            InsertLocation::Auto {},
        )
        .unwrap();
        blob.insert(
            KeyId(kv),
            ValueId(kv),
            &Hash(Bytes32::new([0u8; 32])),
            InsertLocation::Auto {},
        )
        .expect_err("");
    }

    #[rstest]
    fn test_batch_insert(
        #[values(0, 1, 2, 10)] pre_inserts: usize,
        #[values(0, 1, 2, 8, 9)] count: usize,
    ) {
        let mut blob = MerkleBlob::new(vec![]).unwrap();
        for i in 0..pre_inserts {
            let i = i as i64;
            blob.insert(
                KeyId(i),
                ValueId(i),
                &sha256_num(&i),
                InsertLocation::Auto {},
            )
            .unwrap();
        }
        open_dot(blob.to_dot().unwrap().set_note("initial"));

        let mut batch: Vec<((KeyId, ValueId), Hash)> = vec![];

        let mut batch_map: HashMap<KeyId, ValueId> = HashMap::new();
        for i in pre_inserts..(pre_inserts + count) {
            let i = i as i64;
            batch.push(((KeyId(i), ValueId(i)), sha256_num(&i)));
            batch_map.insert(KeyId(i), ValueId(i));
        }

        let before = blob.get_keys_values().unwrap();
        blob.batch_insert(batch).unwrap();
        let after = blob.get_keys_values().unwrap();

        open_dot(
            blob.to_dot()
                .unwrap()
                .set_note(&format!("after batch insert of {count} values")),
        );

        let mut expected = before.clone();
        expected.extend(batch_map);

        assert_eq!(after, expected);
    }

    #[rstest]
    fn test_root_insert_location_when_not_empty(mut small_blob: MerkleBlob) {
        small_blob
            .insert(
                KeyId(0),
                ValueId(0),
                &sha256_num(&0),
                InsertLocation::AsRoot {},
            )
            .expect_err("tree not empty so inserting to root should fail");
    }

    #[rstest]
    fn test_free_index_reused(mut small_blob: MerkleBlob) {
        // there must be enough nodes to avoid the few-node insertion methods that clear the blob
        let count = 5;
        for n in 0..count {
            small_blob
                .insert(
                    KeyId(n),
                    ValueId(n),
                    &sha256_num(&n),
                    InsertLocation::Auto {},
                )
                .unwrap();
        }
        let (key, index) = {
            let (key, index) = small_blob
                .block_status_cache
                .iter_keys_indexes()
                .next()
                .unwrap();
            (*key, *index)
        };
        let expected_length = small_blob.blob.len();
        assert!(!small_blob.block_status_cache.is_index_free(index));
        small_blob.delete(key).unwrap();
        assert!(small_blob.block_status_cache.is_index_free(index));
        let free_indexes = small_blob.block_status_cache.free_indexes.clone();
        assert_eq!(free_indexes.len(), 2);
        let new_index = small_blob
            .insert(
                KeyId(count),
                ValueId(count),
                &sha256_num(&count),
                InsertLocation::Auto {},
            )
            .unwrap();
        assert_eq!(small_blob.blob.len(), expected_length);
        assert!(free_indexes.contains(&new_index));
        assert_eq!(small_blob.block_status_cache.free_index_count(), 0);
    }

    #[rstest]
    fn test_writing_to_free_block_that_contained_an_active_key(small_blob: MerkleBlob) {
        let key = KeyId(0x0001_0203_0405_0607);
        let Some(index) = small_blob.block_status_cache.get_index_by_key(key).copied() else {
            panic!("maybe the test key needs to be updated?")
        };
        let mut prepared_bytes = small_blob.blob.clone();
        prepared_bytes.extend_from_slice(&small_blob.get_block_bytes(index).unwrap());
        let mut prepared_blob = MerkleBlob::new(prepared_bytes).unwrap();
        prepared_blob.check_integrity().unwrap();
        prepared_blob
            .insert(
                KeyId(1),
                ValueId(2),
                &generate_hash(3),
                InsertLocation::Auto {},
            )
            .unwrap();
        assert!(prepared_blob.block_status_cache.contains_key(key));
    }

    #[test]
    fn test_node_expect_leaf_passes() {
        Node::Leaf(LeafNode {
            hash: Hash(Bytes32::default()),
            parent: Parent(None),
            key: KeyId(0),
            value: ValueId(0),
        })
        .expect_leaf("panic message");
    }

    #[test]
    #[should_panic(expected = "panic message")]
    fn test_node_expect_leaf_panics() {
        Node::Internal(InternalNode {
            hash: Hash(Bytes32::default()),
            parent: Parent(None),
            left: TreeIndex(0),
            right: TreeIndex(0),
        })
        .expect_leaf("panic message");
    }

    #[test]
    fn test_node_try_into_leaf_passes() {
        Node::Leaf(LeafNode {
            hash: Hash(Bytes32::default()),
            parent: Parent(None),
            key: KeyId(0),
            value: ValueId(0),
        })
        .try_into_leaf()
        .expect("should pass since it is a leaf");
    }

    #[test]
    fn test_node_try_into_leaf_fails() {
        Node::Internal(InternalNode {
            hash: Hash(Bytes32::default()),
            parent: Parent(None),
            left: TreeIndex(0),
            right: TreeIndex(0),
        })
        .try_into_leaf()
        .expect_err("should fail since it is not a leaf");
    }

    #[test]
    fn test_node_expect_internal_passes() {
        Node::Internal(InternalNode {
            hash: Hash(Bytes32::default()),
            parent: Parent(None),
            left: TreeIndex(0),
            right: TreeIndex(0),
        })
        .expect_internal("panic message");
    }

    #[test]
    #[should_panic(expected = "panic message")]
    fn test_node_expect_internal_panics() {
        Node::Leaf(LeafNode {
            hash: Hash(Bytes32::default()),
            parent: Parent(None),
            key: KeyId(0),
            value: ValueId(0),
        })
        .expect_internal("panic message");
    }

    #[test]
    fn test_internal_node_get_sibling_side_fails_for_non_sibling() {
        let node = InternalNode {
            hash: HASH_ZERO,
            parent: Parent(None),
            left: TreeIndex(1),
            right: TreeIndex(2),
        };
        node.get_sibling_side(TreeIndex(0))
            .expect_err("should fail");
    }

    #[rstest]
    fn test_merkle_blob_to_from_path(traversal_blob: MerkleBlob) {
        let dir_path = tempfile::tempdir().unwrap();
        let file_path = dir_path.path().join("blob");
        traversal_blob.to_path(&file_path).unwrap();
        let loaded = MerkleBlob::from_path(&file_path).unwrap();

        assert!(blob_tree_equality(&traversal_blob, &loaded));
        assert_eq!(traversal_blob.blob, loaded.blob);
    }

    #[rstest]
    fn test_get_node_by_hash(small_blob: MerkleBlob) {
        let node = small_blob.get_node_by_hash(sha256_num(&0x1020)).unwrap();

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            (
                KeyId(
                    283686952306183,
                ),
                ValueId(
                    1157726452361532951,
                ),
            )
        "#]];

        expected.assert_debug_eq(&node);
    }

    #[rstest]
    fn test_get_node_by_hash_fails_not_found(small_blob: MerkleBlob) {
        let result = small_blob.get_node_by_hash(sha256_num(&27));

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            Err(
                LeafHashNotFound(
                    Hash(
                        688e94a51ee508a95e761294afb7a6004b432c15d9890c80ddf23bde8caa4c26,
                    ),
                ),
            )
        "#]];

        expected.assert_debug_eq(&result);
    }

    #[rstest]
    fn test_get_hashes_indexes(small_blob: MerkleBlob) {
        let hashes_indexes = small_blob.get_hashes_indexes(false).unwrap();

        let mut expected = HashMap::new();
        let one = sha256_num(&0x2030);
        let two = sha256_num(&0x1020);
        let zero = internal_hash(&one, &two);
        expected.insert(zero, TreeIndex(0));
        expected.insert(one, TreeIndex(1));
        expected.insert(two, TreeIndex(2));

        assert_eq!(hashes_indexes, expected);
    }

    #[rstest]
    fn test_get_hashes_indexes_leafs_only(small_blob: MerkleBlob) {
        let hashes_indexes = small_blob.get_hashes_indexes(true).unwrap();

        let mut expected = HashMap::new();
        let one = sha256_num(&0x2030);
        let two = sha256_num(&0x1020);
        expected.insert(one, TreeIndex(1));
        expected.insert(two, TreeIndex(2));

        assert_eq!(hashes_indexes, expected);
    }

    #[rstest]
    fn test_get_hashes_indexes_empty() {
        let blob = MerkleBlob::new(Vec::new()).unwrap();
        let result = blob.get_hashes_indexes(false).unwrap();

        assert_eq!(result, HashMap::new());
    }

    #[rstest]
    fn test_node_set_hash(
        #[values(
            Node::Internal(InternalNode{hash: HASH_ZERO, parent: Parent(None), left: TreeIndex(0), right: TreeIndex(1)}),
            Node::Leaf(LeafNode{hash:HASH_ZERO, parent: Parent(None), key: KeyId(0), value: ValueId(0)}),
        )]
        mut node: Node,
    ) {
        assert_eq!(node.hash(), HASH_ZERO);
        node.set_hash(HASH_ONE);
        assert_eq!(node.hash(), HASH_ONE);
    }

    #[rstest]
    fn test_remove_not_present_leaf_from_block_status_cache(mut small_blob: MerkleBlob) {
        let key = KeyId(10948);
        let leaf = LeafNode {
            hash: HASH_ZERO,
            parent: Parent(None),
            key,
            value: ValueId(0),
        };
        let result = small_blob.block_status_cache.remove_leaf(&leaf);

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            Err(
                UnknownKey(
                    KeyId(
                        10948,
                    ),
                ),
            )
        "#]];

        expected.assert_debug_eq(&result);
    }

    #[rstest]
    fn test_insert_past_extend_entry_fails(mut small_blob: MerkleBlob) {
        let index = TreeIndex(small_blob.extend_index().0 + 1);
        let block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: true,
            },
            node: Node::Internal(InternalNode {
                hash: HASH_ZERO,
                parent: Parent(None),
                left: TreeIndex(0),
                right: TreeIndex(0),
            }),
        };
        let error = small_blob.insert_entry_to_blob(index, &block);

        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            Err(
                BlockIndexOutOfBounds(
                    TreeIndex(
                        4,
                    ),
                ),
            )
        "#]];
        expected.assert_debug_eq(&error);
    }

    #[rstest]
    fn test_get_key_index(small_blob: MerkleBlob) {
        let key = KeyId(0x0001_0203_0405_0607);
        let index = small_blob.get_key_index(key).unwrap();
        assert_eq!(index, TreeIndex(2));
    }

    #[rstest]
    fn test_block_status_cache_move_index_invalid_source(mut traversal_blob: MerkleBlob) {
        let key = KeyId(307);
        let index = traversal_blob.get_key_index(key).unwrap();
        traversal_blob.delete(key).unwrap();
        assert!(traversal_blob
            .block_status_cache
            .free_indexes
            .contains(&index));
        let result = traversal_blob.block_status_cache.move_index(index, index);
        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            Err(
                MoveSourceIndexNotInUse(
                    TreeIndex(
                        5,
                    ),
                ),
            )
        "#]];

        expected.assert_debug_eq(&result);
    }

    #[rstest]
    fn test_block_status_cache_move_index_invalid_destination(mut traversal_blob: MerkleBlob) {
        let key = KeyId(307);
        let index = traversal_blob.get_key_index(key).unwrap();
        traversal_blob.delete(key).unwrap();
        assert!(traversal_blob
            .block_status_cache
            .free_indexes
            .contains(&index));
        let result = traversal_blob
            .block_status_cache
            .move_index(TreeIndex(0), index);
        #[allow(clippy::needless_raw_string_hashes)]
        let expected = expect![[r#"
            Err(
                MoveDestinationIndexNotInUse(
                    TreeIndex(
                        5,
                    ),
                ),
            )
        "#]];

        expected.assert_debug_eq(&result);
    }

    #[rstest]
    fn test_moved_sibling_retains_hash(mut small_blob: MerkleBlob) {
        let key_to_delete = KeyId(0x0001_0203_0405_0607);
        let remaining_hash = sha256_num(&0x2030);
        assert_ne!(small_blob.get_hash(TreeIndex(0)).unwrap(), remaining_hash);
        small_blob.delete(key_to_delete).unwrap();
        assert_eq!(small_blob.get_hash(TreeIndex(0)).unwrap(), remaining_hash);
    }
}
