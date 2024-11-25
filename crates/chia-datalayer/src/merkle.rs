#[cfg(feature = "py-bindings")]
use pyo3::{
    buffer::PyBuffer, exceptions::PyValueError, pyclass, pymethods, FromPyObject, IntoPy, PyObject,
    PyResult, Python,
};

use chia_protocol::Bytes32;
use chia_streamable_macro::Streamable;
use chia_traits::Streamable;
use clvmr::sha2::Sha256;
use num_traits::ToBytes;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::iter::zip;
use std::ops::Range;
use thiserror::Error;

#[cfg_attr(feature = "py-bindings", derive(FromPyObject), pyo3(transparent))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct TreeIndex(u32);

#[cfg(feature = "py-bindings")]
impl IntoPy<PyObject> for TreeIndex {
    fn into_py(self, py: Python<'_>) -> PyObject {
        self.0.into_py(py)
    }
}

impl std::fmt::Display for TreeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

type Parent = Option<TreeIndex>;
type Hash = Bytes32;
/// Key and value ids are provided from outside of this code and are implemented as
/// the row id from sqlite which is a signed 8 byte integer.  The actual key and
/// value data bytes will not be handled within this code, only outside.
#[cfg_attr(feature = "py-bindings", derive(FromPyObject), pyo3(transparent))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct KvId(i64);

#[cfg(feature = "py-bindings")]
impl IntoPy<PyObject> for KvId {
    fn into_py(self, py: Python<'_>) -> PyObject {
        self.0.into_py(py)
    }
}

impl std::fmt::Display for KvId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("unknown NodeType value: {0:?}")]
    UnknownNodeTypeValue(u8),

    #[error("unknown dirty value: {0:?}")]
    UnknownDirtyValue(u8),

    // TODO: don't use String here
    #[error("failed loading metadata: {0}")]
    FailedLoadingMetadata(String),

    // TODO: don't use String here
    #[error("failed loading node: {0}")]
    FailedLoadingNode(String),

    #[error("blob length must be a multiple of block count, found extra bytes: {0}")]
    InvalidBlobLength(usize),

    #[error("key already present")]
    KeyAlreadyPresent,

    #[error("requested insertion at root but tree not empty")]
    UnableToInsertAsRootOfNonEmptyTree,

    #[error("unable to find a leaf")]
    UnableToFindALeaf,

    #[error("unknown key: {0:?}")]
    UnknownKey(KvId),

    #[error("key not in key to index cache: {0:?}")]
    IntegrityKeyNotInCache(KvId),

    #[error("zero-length seed bytes not allowed")]
    ZeroLengthSeedNotAllowed,

    #[error("block index out of range: {0:?}")]
    BlockIndexOutOfRange(TreeIndex),

    #[error("node not a leaf: {0:?}")]
    NodeNotALeaf(InternalNode),

    #[error("from streamable: {0:?}")]
    Streaming(chia_traits::chia_error::Error),

    #[error("index not a child: {0}")]
    IndexIsNotAChild(TreeIndex),
}

// assumptions
// - root is at index 0
// - any case with no keys will have a zero length blob

// define the serialized block format
const METADATA_RANGE: Range<usize> = 0..METADATA_SIZE;
const METADATA_SIZE: usize = 2;
// TODO: figure out the real max better than trial and error?
const DATA_SIZE: usize = 53;
const BLOCK_SIZE: usize = METADATA_SIZE + DATA_SIZE;
type BlockBytes = [u8; BLOCK_SIZE];
type MetadataBytes = [u8; METADATA_SIZE];
type DataBytes = [u8; DATA_SIZE];
const DATA_RANGE: Range<usize> = METADATA_SIZE..METADATA_SIZE + DATA_SIZE;

fn streamable_from_bytes_ignore_extra_bytes<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: Streamable,
{
    let mut cursor = std::io::Cursor::new(bytes);
    // TODO: consider trusted mode?
    T::parse::<false>(&mut cursor).map_err(Error::Streaming)
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Streamable)]
pub enum NodeType {
    Internal = 0,
    Leaf = 1,
}

impl NodeType {
    pub fn from_u8(value: u8) -> Result<Self, Error> {
        streamable_from_bytes_ignore_extra_bytes(&[value])
    }

    #[allow(clippy::wrong_self_convention, clippy::trivially_copy_pass_by_ref)]
    pub fn to_u8(&self) -> u8 {
        Streamable::to_bytes(self).unwrap()[0]
    }
}

#[allow(clippy::needless_pass_by_value)]
fn sha256_num<T: ToBytes>(input: T) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input.to_be_bytes());

    Bytes32::new(hasher.finalize())
}

fn sha256_bytes(input: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input);

    Bytes32::new(hasher.finalize())
}

fn internal_hash(left_hash: &Hash, right_hash: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(b"\x02");
    hasher.update(left_hash);
    hasher.update(right_hash);

    Bytes32::new(hasher.finalize())
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "Side", eq, eq_int))]
#[repr(u8)]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Side {
    Left = 0,
    Right = 1,
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "InsertLocation"))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum InsertLocation {
    // error: Unit variant `Auto` is not yet supported in a complex enum
    // = help: change to a struct variant with no fields: `Auto { }`
    // = note: the enum is complex because of non-unit variant `Leaf`
    Auto {},
    AsRoot {},
    Leaf { index: TreeIndex, side: Side },
}

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, Streamable)]
pub struct NodeMetadata {
    // OPT: could save 1-2% of tree space by packing (and maybe don't do that)
    pub node_type: NodeType,
    pub dirty: bool,
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "InternalNode", get_all))]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Streamable)]
pub struct InternalNode {
    parent: Parent,
    hash: Hash,
    left: TreeIndex,
    right: TreeIndex,
}

impl InternalNode {
    pub fn sibling_index(&self, index: TreeIndex) -> Result<TreeIndex, Error> {
        if index == self.right {
            Ok(self.left)
        } else if index == self.left {
            Ok(self.right)
        } else {
            Err(Error::IndexIsNotAChild(index))
        }
    }
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "LeafNode", get_all))]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Streamable)]
pub struct LeafNode {
    parent: Parent,
    hash: Hash,
    key: KvId,
    value: KvId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Internal(InternalNode),
    Leaf(LeafNode),
}

impl Node {
    fn parent(&self) -> Parent {
        match self {
            Node::Internal(node) => node.parent,
            Node::Leaf(node) => node.parent,
        }
    }

    fn set_parent(&mut self, parent: Parent) {
        match self {
            Node::Internal(node) => node.parent = parent,
            Node::Leaf(node) => node.parent = parent,
        }
    }

    fn hash(&self) -> Hash {
        match self {
            Node::Internal(node) => node.hash,
            Node::Leaf(node) => node.hash,
        }
    }

    fn set_hash(&mut self, hash: &Hash) {
        match self {
            Node::Internal(ref mut node) => node.hash = *hash,
            Node::Leaf(ref mut node) => node.hash = *hash,
        }
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn from_bytes(metadata: &NodeMetadata, blob: &DataBytes) -> Result<Self, Error> {
        Ok(match metadata.node_type {
            NodeType::Internal => Node::Internal(streamable_from_bytes_ignore_extra_bytes(blob)?),
            NodeType::Leaf => Node::Leaf(streamable_from_bytes_ignore_extra_bytes(blob)?),
        })
    }

    pub fn to_bytes(&self) -> Result<DataBytes, Error> {
        let mut base = match self {
            Node::Internal(node) => node.to_bytes(),
            Node::Leaf(node) => node.to_bytes(),
        }
        .map_err(Error::Streaming)?;
        assert!(base.len() <= DATA_SIZE);
        base.resize(DATA_SIZE, 0);
        Ok(base
            .as_slice()
            .try_into()
            .expect("padding was added above, might be too large"))
    }

    fn expect_leaf(&self, message: &str) -> LeafNode {
        let Node::Leaf(leaf) = self else {
            let message = message.replace("<<self>>", &format!("{self:?}"));
            panic!("{}", message)
        };

        *leaf
    }

    fn try_into_leaf(self) -> Result<LeafNode, Error> {
        match self {
            Node::Leaf(leaf) => Ok(leaf),
            Node::Internal(internal) => Err(Error::NodeNotALeaf(internal)),
        }
    }
}

#[cfg(feature = "py-bindings")]
impl IntoPy<PyObject> for Node {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Node::Internal(node) => node.into_py(py),
            Node::Leaf(node) => node.into_py(py),
        }
    }
}

fn block_range(index: TreeIndex) -> Range<usize> {
    let block_start = index.0 as usize * BLOCK_SIZE;
    block_start..block_start + BLOCK_SIZE
}

pub struct Block {
    // TODO: metadata node type and node's type not verified for agreement
    metadata: NodeMetadata,
    node: Node,
}

impl Block {
    pub fn to_bytes(&self) -> Result<BlockBytes, Error> {
        let mut blob: BlockBytes = [0; BLOCK_SIZE];
        blob[METADATA_RANGE].copy_from_slice(&self.metadata.to_bytes().map_err(Error::Streaming)?);
        blob[DATA_RANGE].copy_from_slice(&self.node.to_bytes()?);

        Ok(blob)
    }

    pub fn from_bytes(blob: BlockBytes) -> Result<Self, Error> {
        let metadata_blob: MetadataBytes = blob[METADATA_RANGE].try_into().unwrap();
        let data_blob: DataBytes = blob[DATA_RANGE].try_into().unwrap();
        let metadata = NodeMetadata::from_bytes(&metadata_blob)
            .map_err(|message| Error::FailedLoadingMetadata(message.to_string()))?;
        let node = Node::from_bytes(&metadata, &data_blob)
            .map_err(|message| Error::FailedLoadingNode(message.to_string()))?;

        Ok(Block { metadata, node })
    }

    pub fn update_hash(&mut self, left: &Hash, right: &Hash) {
        self.node.set_hash(&internal_hash(left, right));
        self.metadata.dirty = false;
    }
}

fn get_free_indexes_and_keys_values_indexes(
    blob: &[u8],
) -> (HashSet<TreeIndex>, HashMap<KvId, TreeIndex>) {
    let index_count = blob.len() / BLOCK_SIZE;

    let mut seen_indexes: Vec<bool> = vec![false; index_count];
    let mut key_to_index: HashMap<KvId, TreeIndex> = HashMap::default();

    for (index, block) in MerkleBlobLeftChildFirstIterator::new(blob) {
        seen_indexes[index.0 as usize] = true;

        if let Node::Leaf(leaf) = block.node {
            key_to_index.insert(leaf.key, index);
        }
    }

    let mut free_indexes: HashSet<TreeIndex> = HashSet::new();
    for (index, seen) in seen_indexes.iter().enumerate() {
        if !seen {
            free_indexes.insert(TreeIndex(index as u32));
        }
    }

    (free_indexes, key_to_index)
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "MerkleBlob", get_all))]
#[derive(Debug)]
pub struct MerkleBlob {
    blob: Vec<u8>,
    free_indexes: HashSet<TreeIndex>,
    key_to_index: HashMap<KvId, TreeIndex>,
}

impl MerkleBlob {
    pub fn new(blob: Vec<u8>) -> Result<Self, Error> {
        let length = blob.len();
        let remainder = length % BLOCK_SIZE;
        if remainder != 0 {
            return Err(Error::InvalidBlobLength(remainder));
        }

        let (free_indexes, key_to_index) = get_free_indexes_and_keys_values_indexes(&blob);

        Ok(Self {
            blob,
            free_indexes,
            key_to_index,
        })
    }

    fn clear(&mut self) {
        self.blob.clear();
        self.key_to_index.clear();
        self.free_indexes.clear();
    }

    pub fn insert(
        &mut self,
        key: KvId,
        value: KvId,
        hash: &Hash,
        insert_location: InsertLocation,
    ) -> Result<(), Error> {
        if self.key_to_index.contains_key(&key) {
            return Err(Error::KeyAlreadyPresent);
        }

        let insert_location = match insert_location {
            InsertLocation::Auto {} => self.get_random_insert_location_by_kvid(key)?,
            _ => insert_location,
        };

        match insert_location {
            InsertLocation::Auto {} => {
                unreachable!("this should have been caught and processed above")
            }
            InsertLocation::AsRoot {} => {
                if !self.key_to_index.is_empty() {
                    return Err(Error::UnableToInsertAsRootOfNonEmptyTree);
                };
                self.insert_first(key, value, hash)?;
            }
            InsertLocation::Leaf { index, side } => {
                let old_leaf = self.get_node(index)?.try_into_leaf()?;

                let internal_node_hash = match side {
                    Side::Left => internal_hash(hash, &old_leaf.hash),
                    Side::Right => internal_hash(&old_leaf.hash, hash),
                };

                let node = LeafNode {
                    parent: None,
                    hash: *hash,
                    key,
                    value,
                };

                if self.key_to_index.len() == 1 {
                    self.insert_second(node, &old_leaf, &internal_node_hash, &side)?;
                } else {
                    self.insert_third_or_later(node, &old_leaf, index, &internal_node_hash, &side)?;
                }
            }
        }

        Ok(())
    }

    fn insert_first(&mut self, key: KvId, value: KvId, hash: &Hash) -> Result<(), Error> {
        let new_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node::Leaf(LeafNode {
                parent: None,
                key,
                value,
                hash: *hash,
            }),
        };

        self.clear();
        self.insert_entry_to_blob(self.extend_index(), &new_leaf_block)?;

        Ok(())
    }

    fn insert_second(
        &mut self,
        mut node: LeafNode,
        old_leaf: &LeafNode,
        internal_node_hash: &Hash,
        side: &Side,
    ) -> Result<(), Error> {
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
                parent: None,
                left: left_index,
                right: right_index,
                hash: *internal_node_hash,
            }),
        };

        self.insert_entry_to_blob(root_index, &new_internal_block)?;

        node.parent = Some(TreeIndex(0));

        let nodes = [
            (
                match side {
                    Side::Left => right_index,
                    Side::Right => left_index,
                },
                LeafNode {
                    parent: Some(TreeIndex(0)),
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

        Ok(())
    }

    fn insert_third_or_later(
        &mut self,
        mut node: LeafNode,
        old_leaf: &LeafNode,
        old_leaf_index: TreeIndex,
        internal_node_hash: &Hash,
        side: &Side,
    ) -> Result<(), Error> {
        let new_leaf_index = self.get_new_index();
        let new_internal_node_index = self.get_new_index();

        node.parent = Some(new_internal_node_index);

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

        let Some(old_parent_index) = old_leaf.parent else {
            panic!("root found when not expected")
        };

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
        };

        self.insert_entry_to_blob(old_parent_index, &old_parent_block)?;

        self.mark_lineage_as_dirty(old_parent_index)?;

        Ok(())
    }

    pub fn batch_insert<I>(&mut self, mut keys_values_hashes: I) -> Result<(), Error>
    where
        I: Iterator<Item = ((KvId, KvId), Hash)>,
    {
        // OPT: would it be worthwhile to hold the entire blocks?
        let mut indexes = vec![];

        if self.key_to_index.len() <= 1 {
            for _ in 0..2 {
                let Some(((key, value), hash)) = keys_values_hashes.next() else {
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
                    parent: None,
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
                        parent: None,
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
            self.insert_from_key(min_height_leaf.key, indexes[0], &Side::Left)?;
        };

        Ok(())
    }

    fn insert_from_key(
        &mut self,
        old_leaf_key: KvId,
        new_index: TreeIndex,
        side: &Side,
    ) -> Result<(), Error> {
        // NAME: consider name, we're inserting a subtree at a leaf
        // TODO: seems like this ought to be fairly similar to regular insert

        // TODO: but what about the old leaf being the root...  is that what the batch insert
        //       pre-filling of two leafs is about?  if so, this needs to be making sure of that
        //       or something.

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

        let Some(old_leaf_parent) = old_leaf.parent else {
            // TODO: relates to comment at the beginning about assumptions about the tree etc
            panic!("not handling this case");
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
        self.update_parent(old_leaf_index, Some(new_internal_node_index))?;

        Ok(())
    }

    fn get_min_height_leaf(&self) -> Result<LeafNode, Error> {
        let block = MerkleBlobBreadthFirstIterator::new(&self.blob)
            .next()
            .ok_or(Error::UnableToFindALeaf)?;

        Ok(block
            .node
            .expect_leaf("unexpectedly found internal node first: <<self>>"))
    }

    pub fn delete(&mut self, key: KvId) -> Result<(), Error> {
        let (leaf_index, leaf, _leaf_block) = self.get_leaf_by_key(key)?;
        self.key_to_index.remove(&key);

        let Some(parent_index) = leaf.parent else {
            self.clear();
            return Ok(());
        };

        self.free_indexes.insert(leaf_index);
        let maybe_parent = self.get_node(parent_index)?;
        let Node::Internal(parent) = maybe_parent else {
            panic!("parent node not internal: {maybe_parent:?}")
        };
        let sibling_index = parent.sibling_index(leaf_index)?;
        let mut sibling_block = self.get_block(sibling_index)?;

        let Some(grandparent_index) = parent.parent else {
            sibling_block.node.set_parent(None);
            self.insert_entry_to_blob(TreeIndex(0), &sibling_block)?;

            if let Node::Internal(node) = sibling_block.node {
                for child_index in [node.left, node.right] {
                    self.update_parent(child_index, Some(TreeIndex(0)))?;
                }
            };

            self.free_indexes.insert(sibling_index);

            return Ok(());
        };

        self.free_indexes.insert(parent_index);
        let mut grandparent_block = self.get_block(grandparent_index)?;

        sibling_block.node.set_parent(Some(grandparent_index));
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

    pub fn upsert(&mut self, key: KvId, value: KvId, new_hash: &Hash) -> Result<(), Error> {
        let Ok((leaf_index, mut leaf, mut block)) = self.get_leaf_by_key(key) else {
            self.insert(key, value, new_hash, InsertLocation::Auto {})?;
            return Ok(());
        };

        leaf.hash.clone_from(new_hash);
        leaf.value = value;
        // OPT: maybe just edit in place?
        block.node = Node::Leaf(leaf);
        self.insert_entry_to_blob(leaf_index, &block)?;

        if let Some(parent) = block.node.parent() {
            self.mark_lineage_as_dirty(parent)?;
        }

        Ok(())
    }

    pub fn check_integrity(&self) -> Result<(), Error> {
        let mut leaf_count: usize = 0;
        let mut internal_count: usize = 0;
        let mut child_to_parent: HashMap<TreeIndex, TreeIndex> = HashMap::new();

        for (index, block) in MerkleBlobParentFirstIterator::new(&self.blob) {
            if let Some(parent) = block.node.parent() {
                assert_eq!(child_to_parent.remove(&index), Some(parent));
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
                        .key_to_index
                        .get(&node.key)
                        .ok_or(Error::IntegrityKeyNotInCache(node.key))?;
                    let key = node.key;
                    assert_eq!(
                        *cached_index, index,
                        "key to index cache for {key:?} should be {index:?} got: {cached_index:?}"
                    );
                    assert!(
                        !self.free_indexes.contains(&index),
                        "{}",
                        format!("active index found in free index list: {index:?}")
                    );
                }
            }
        }

        let key_to_index_cache_length = self.key_to_index.len();
        assert_eq!(leaf_count, key_to_index_cache_length, "found {leaf_count:?} leaves but key to index cache length is: {key_to_index_cache_length:?}");
        let total_count = leaf_count + internal_count + self.free_indexes.len();
        let extend_index = self.extend_index();
        assert_eq!(
            total_count, extend_index.0 as usize,
            "expected total node count {extend_index:?} found: {total_count:?}",
        );
        assert_eq!(child_to_parent.len(), 0);

        Ok(())
    }

    fn update_parent(
        &mut self,
        index: TreeIndex,
        parent: Option<TreeIndex>,
    ) -> Result<Block, Error> {
        let mut block = self.get_block(index)?;
        block.node.set_parent(parent);
        self.insert_entry_to_blob(index, &block)?;

        Ok(block)
    }

    fn mark_lineage_as_dirty(&mut self, index: TreeIndex) -> Result<(), Error> {
        let mut next_index = Some(index);

        while let Some(this_index) = next_index {
            let mut block = Block::from_bytes(self.get_block_bytes(this_index)?)?;

            if block.metadata.dirty {
                return Ok(());
            }

            block.metadata.dirty = true;
            self.insert_entry_to_blob(this_index, &block)?;
            next_index = block.node.parent();
        }

        Ok(())
    }

    fn get_new_index(&mut self) -> TreeIndex {
        match self.free_indexes.iter().next().copied() {
            None => {
                let index = self.extend_index();
                self.blob.extend_from_slice(&[0; BLOCK_SIZE]);
                // NOTE: explicitly not marking index as free since that would hazard two
                //       sequential calls to this function through this path to both return
                //       the same index
                index
            }
            Some(new_index) => {
                self.free_indexes.remove(&new_index);
                new_index
            }
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

        let side = if (seed_bytes.last().ok_or(Error::ZeroLengthSeedNotAllowed)? & 1 << 7) == 0 {
            Side::Left
        } else {
            Side::Right
        };
        let mut next_index = TreeIndex(0);
        let mut node = self.get_node(next_index)?;

        loop {
            for byte in &seed_bytes {
                for bit in 0..8 {
                    match node {
                        Node::Leaf { .. } => {
                            return Ok(InsertLocation::Leaf {
                                index: next_index,
                                side,
                            })
                        }
                        Node::Internal(internal) => {
                            next_index = if byte & (1 << bit) != 0 {
                                internal.left
                            } else {
                                internal.right
                            };
                            node = self.get_node(next_index)?;
                        }
                    }
                }
            }

            seed_bytes = sha256_bytes(&seed_bytes).into();
        }
    }

    fn get_random_insert_location_by_kvid(&self, seed: KvId) -> Result<InsertLocation, Error> {
        let seed = sha256_num(seed.0);

        self.get_random_insert_location_by_seed(&seed)
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
            Ordering::Greater => return Err(Error::BlockIndexOutOfRange(index)),
            Ordering::Equal => self.blob.extend_from_slice(&new_block_bytes),
            Ordering::Less => {
                // OPT: lots of deserialization here for just the key
                let old_block = self.get_block(index)?;
                // TODO: should we be more careful about accidentally reading garbage like
                //       from a freshly gotten index
                if !self.free_indexes.contains(&index)
                    && old_block.metadata.node_type == NodeType::Leaf
                {
                    if let Node::Leaf(old_node) = old_block.node {
                        self.key_to_index.remove(&old_node.key);
                    };
                };
                self.blob[block_range(index)].copy_from_slice(&new_block_bytes);
            }
        }

        if let Node::Leaf(ref node) = block.node {
            self.key_to_index.insert(node.key, index);
        };

        self.free_indexes.take(&index);

        Ok(())
    }

    fn get_block(&self, index: TreeIndex) -> Result<Block, Error> {
        Block::from_bytes(self.get_block_bytes(index)?)
    }

    fn get_hash(&self, index: TreeIndex) -> Result<Hash, Error> {
        Ok(self.get_block(index)?.node.hash())
    }

    fn get_block_bytes(&self, index: TreeIndex) -> Result<BlockBytes, Error> {
        Ok(self
            .blob
            .get(block_range(index))
            .ok_or(Error::BlockIndexOutOfRange(index))?
            .try_into()
            .unwrap_or_else(|e| panic!("failed getting block {index}: {e}")))
    }

    pub fn get_node(&self, index: TreeIndex) -> Result<Node, Error> {
        Ok(self.get_block(index)?.node)
    }

    pub fn get_leaf_by_key(&self, key: KvId) -> Result<(TreeIndex, LeafNode, Block), Error> {
        let index = *self.key_to_index.get(&key).ok_or(Error::UnknownKey(key))?;
        let block = self.get_block(index)?;
        let leaf = block.node.expect_leaf(&format!(
            "expected leaf for index from key cache: {index} -> <<self>>"
        ));

        Ok((index, leaf, block))
    }

    pub fn get_parent_index(&self, index: TreeIndex) -> Result<Parent, Error> {
        Ok(self.get_block(index)?.node.parent())
    }

    pub fn get_lineage_with_indexes(
        &self,
        index: TreeIndex,
    ) -> Result<Vec<(TreeIndex, Node)>, Error> {
        let mut next_index = Some(index);
        let mut lineage = vec![];

        while let Some(this_index) = next_index {
            let node = self.get_node(this_index)?;
            next_index = node.parent();
            lineage.push((index, node));
        }

        Ok(lineage)
    }

    pub fn get_lineage_indexes(&self, index: TreeIndex) -> Result<Vec<TreeIndex>, Error> {
        let mut next_index = Some(index);
        let mut lineage: Vec<TreeIndex> = vec![];

        while let Some(this_index) = next_index {
            lineage.push(this_index);
            next_index = self.get_parent_index(this_index)?;
        }

        Ok(lineage)
    }

    // pub fn iter(&self) -> MerkleBlobLeftChildFirstIterator<'_> {
    //     <&Self as IntoIterator>::into_iter(self)
    // }

    pub fn calculate_lazy_hashes(&mut self) -> Result<(), Error> {
        // OPT: really want a truncated traversal, not filter
        // OPT: yeah, storing the whole set of blocks via collect is not great
        for (index, mut block) in MerkleBlobLeftChildFirstIterator::new(&self.blob)
            .filter(|(_, block)| block.metadata.dirty)
            .collect::<Vec<_>>()
        {
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
}

impl PartialEq for MerkleBlob {
    fn eq(&self, other: &Self) -> bool {
        // NOTE: this is checking tree structure equality, not serialized bytes equality
        for ((_, self_block), (_, other_block)) in zip(
            MerkleBlobLeftChildFirstIterator::new(&self.blob),
            MerkleBlobLeftChildFirstIterator::new(&other.blob),
        ) {
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
}

// impl<'a> IntoIterator for &'a MerkleBlob {
//     type Item = (TreeIndex, Block);
//     type IntoIter = MerkleBlobLeftChildFirstIterator<'a>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         MerkleBlobLeftChildFirstIterator::new(&self.blob)
//     }
// }

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

        Self::new(Vec::from(slice)).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[pyo3(name = "insert", signature = (key, value, hash, reference_kid = None, side = None))]
    pub fn py_insert(
        &mut self,
        key: KvId,
        value: KvId,
        hash: Hash,
        reference_kid: Option<KvId>,
        // TODO: should be a Side, but python has a different Side right now
        side: Option<u8>,
    ) -> PyResult<()> {
        let insert_location = match (reference_kid, side) {
            (None, None) => InsertLocation::Auto {},
            (Some(key), Some(side)) => InsertLocation::Leaf {
                index: *self
                    .key_to_index
                    .get(&key)
                    .ok_or(PyValueError::new_err(format!(
                        "unknown key id passed as insert location reference: {key}"
                    )))?,
                side: match side {
                    x if x == (Side::Left as u8) => Side::Left,
                    x if x == (Side::Right as u8) => Side::Right,
                    _ => panic!(),
                },
            },
            _ => {
                return Err(PyValueError::new_err(
                    "must specify neither or both of reference_kid and side",
                ));
            }
        };
        self.insert(key, value, &hash, insert_location)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[pyo3(name = "delete")]
    pub fn py_delete(&mut self, key: KvId) -> PyResult<()> {
        self.delete(key)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[pyo3(name = "get_raw_node")]
    pub fn py_get_raw_node(&mut self, index: TreeIndex) -> PyResult<Node> {
        self.get_node(index)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[pyo3(name = "calculate_lazy_hashes")]
    pub fn py_calculate_lazy_hashes(&mut self) -> PyResult<()> {
        self.calculate_lazy_hashes()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    #[pyo3(name = "get_lineage_with_indexes")]
    pub fn py_get_lineage_with_indexes(
        &self,
        index: TreeIndex,
        py: Python<'_>,
    ) -> PyResult<pyo3::PyObject> {
        let list = pyo3::types::PyList::empty_bound(py);

        for (index, node) in self
            .get_lineage_with_indexes(index)
            .map_err(|e| PyValueError::new_err(e.to_string()))?
        {
            use pyo3::conversion::IntoPy;
            use pyo3::types::PyListMethods;
            list.append((index.into_py(py), node.into_py(py)))?;
        }

        Ok(list.into())
    }

    #[pyo3(name = "get_nodes_with_indexes")]
    pub fn py_get_nodes_with_indexes(&self, py: Python<'_>) -> PyResult<pyo3::PyObject> {
        let list = pyo3::types::PyList::empty_bound(py);

        for (index, block) in MerkleBlobParentFirstIterator::new(&self.blob) {
            use pyo3::conversion::IntoPy;
            use pyo3::types::PyListMethods;
            list.append((index.into_py(py), block.node.into_py(py)))?;
        }

        Ok(list.into())
    }

    #[pyo3(name = "empty")]
    pub fn py_empty(&self) -> PyResult<bool> {
        Ok(self.key_to_index.is_empty())
    }

    #[pyo3(name = "get_root_hash")]
    pub fn py_get_root_hash(&self) -> PyResult<Option<Hash>> {
        self.py_get_hash_at_index(TreeIndex(0))
    }

    #[pyo3(name = "get_hash_at_index")]
    pub fn py_get_hash_at_index(&self, index: TreeIndex) -> PyResult<Option<Hash>> {
        if self.key_to_index.is_empty() {
            return Ok(None);
        }

        let block = self
            .get_block(index)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        if block.metadata.dirty {
            return Err(PyValueError::new_err("root hash is dirty"));
        }

        Ok(Some(block.node.hash()))
    }

    #[pyo3(name = "batch_insert")]
    pub fn py_batch_insert(
        &mut self,
        keys_values: Vec<(KvId, KvId)>,
        hashes: Vec<Hash>,
    ) -> PyResult<()> {
        if keys_values.len() != hashes.len() {
            return Err(PyValueError::new_err(
                "key/value and hash collection lengths must match",
            ));
        }

        self.batch_insert(&mut zip(keys_values, hashes))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(())
    }

    #[pyo3(name = "__len__")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.blob.len())
    }
}

struct MerkleBlobLeftChildFirstIteratorItem {
    visited: bool,
    index: TreeIndex,
}

pub struct MerkleBlobLeftChildFirstIterator<'a> {
    blob: &'a [u8],
    deque: VecDeque<MerkleBlobLeftChildFirstIteratorItem>,
}

impl<'a> MerkleBlobLeftChildFirstIterator<'a> {
    fn new(blob: &'a [u8]) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(MerkleBlobLeftChildFirstIteratorItem {
                visited: false,
                index: TreeIndex(0),
            });
        }

        Self { blob, deque }
    }
}

impl Iterator for MerkleBlobLeftChildFirstIterator<'_> {
    type Item = (TreeIndex, Block);

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, children before parents

        loop {
            let item = self.deque.pop_front()?;
            let block_bytes: BlockBytes = self.blob[block_range(item.index)].try_into().unwrap();
            let block = Block::from_bytes(block_bytes).unwrap();

            match block.node {
                Node::Leaf(..) => return Some((item.index, block)),
                Node::Internal(ref node) => {
                    if item.visited {
                        return Some((item.index, block));
                    };

                    self.deque.push_front(MerkleBlobLeftChildFirstIteratorItem {
                        visited: true,
                        index: item.index,
                    });
                    self.deque.push_front(MerkleBlobLeftChildFirstIteratorItem {
                        visited: false,
                        index: node.right,
                    });
                    self.deque.push_front(MerkleBlobLeftChildFirstIteratorItem {
                        visited: false,
                        index: node.left,
                    });
                }
            }
        }
    }
}

pub struct MerkleBlobParentFirstIterator<'a> {
    blob: &'a [u8],
    deque: VecDeque<TreeIndex>,
}

impl<'a> MerkleBlobParentFirstIterator<'a> {
    fn new(blob: &'a [u8]) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(TreeIndex(0));
        }

        Self { blob, deque }
    }
}

impl Iterator for MerkleBlobParentFirstIterator<'_> {
    type Item = (TreeIndex, Block);

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parents before children

        let index = self.deque.pop_front()?;
        let block_bytes: BlockBytes = self.blob[block_range(index)].try_into().unwrap();
        let block = Block::from_bytes(block_bytes).unwrap();

        if let Node::Internal(ref node) = block.node {
            self.deque.push_back(node.left);
            self.deque.push_back(node.right);
        }

        Some((index, block))
    }
}

pub struct MerkleBlobBreadthFirstIterator<'a> {
    blob: &'a [u8],
    deque: VecDeque<TreeIndex>,
}

impl<'a> MerkleBlobBreadthFirstIterator<'a> {
    #[allow(unused)]
    fn new(blob: &'a [u8]) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(TreeIndex(0));
        }

        Self { blob, deque }
    }
}

impl Iterator for MerkleBlobBreadthFirstIterator<'_> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parent depth before child depth

        loop {
            let index = self.deque.pop_front()?;
            let block_bytes: BlockBytes = self.blob[block_range(index)].try_into().unwrap();
            let block = Block::from_bytes(block_bytes).unwrap();

            match block.node {
                Node::Leaf(..) => return Some(block),
                Node::Internal(node) => {
                    self.deque.push_back(node.left);
                    self.deque.push_back(node.right);
                }
            }
        }
    }
}

#[cfg(any(test, debug_assertions))]
impl Drop for MerkleBlob {
    fn drop(&mut self) {
        self.check_integrity().unwrap();
    }
}

#[cfg(test)]
mod dot;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::dot::DotLines;
    use rstest::{fixture, rstest};
    use std::time::{Duration, Instant};

    fn open_dot(_lines: &mut DotLines) {
        // crate::merkle::dot::open_dot(_lines);
    }

    impl MerkleBlob {
        fn get_key_value_map(&self) -> HashMap<KvId, KvId> {
            let mut key_value = HashMap::new();
            for key in self.key_to_index.keys() {
                // silly waste of having the index, but test code and type narrowing so, ok i guess
                let (_leaf_index, leaf, _leaf_block) = self.get_leaf_by_key(*key).unwrap();
                key_value.insert(*key, leaf.value);
            }

            key_value
        }
    }

    #[test]
    fn test_node_type_serialized_values() {
        assert_eq!(NodeType::Internal as u8, 0);
        assert_eq!(NodeType::Leaf as u8, 1);

        for node_type in [NodeType::Internal, NodeType::Leaf] {
            assert_eq!(node_type.to_u8(), node_type as u8,);
            assert_eq!(NodeType::from_u8(node_type as u8).unwrap(), node_type,);
        }
    }

    #[test]
    fn test_internal_hash() {
        // in Python: Program.to((left_hash, right_hash)).get_tree_hash_precalc(left_hash, right_hash)

        let left: Hash = (0u8..32).collect::<Vec<_>>().try_into().unwrap();
        let right: Hash = (32u8..64).collect::<Vec<_>>().try_into().unwrap();

        assert_eq!(
            internal_hash(&left, &right),
            Bytes32::new(
                clvm_utils::tree_hash_pair(
                    clvm_utils::TreeHash::new(left.to_bytes()),
                    clvm_utils::TreeHash::new(right.to_bytes()),
                )
                .to_bytes()
            ),
        );
    }

    #[rstest]
    fn test_node_metadata_from_to(
        #[values(false, true)] dirty: bool,
        #[values(NodeType::Internal, NodeType::Leaf)] node_type: NodeType,
    ) {
        let bytes: [u8; 2] = [node_type.to_u8(), dirty as u8];
        let object = NodeMetadata::from_bytes(&bytes).unwrap();
        assert_eq!(object, NodeMetadata { node_type, dirty },);
        assert_eq!(object.to_bytes().unwrap(), bytes);
    }

    #[fixture]
    fn small_blob() -> MerkleBlob {
        let mut blob = MerkleBlob::new(vec![]).unwrap();

        blob.insert(
            KvId(0x0001_0203_0405_0607),
            KvId(0x1011_1213_1415_1617),
            &sha256_num(0x1020),
            InsertLocation::Auto {},
        )
        .unwrap();

        blob.insert(
            KvId(0x2021_2223_2425_2627),
            KvId(0x3031_3233_3435_3637),
            &sha256_num(0x2030),
            InsertLocation::Auto {},
        )
        .unwrap();

        blob
    }

    #[rstest]
    fn test_get_lineage(small_blob: MerkleBlob) {
        let lineage = small_blob.get_lineage_with_indexes(TreeIndex(2)).unwrap();
        for (_, node) in &lineage {
            println!("{node:?}");
        }
        assert_eq!(lineage.len(), 2);
        let (_, last_node) = lineage.last().unwrap();
        assert_eq!(last_node.parent(), None);
    }

    #[rstest]
    #[case::right(0, TreeIndex(2), Side::Left)]
    #[case::left(0xff, TreeIndex(1), Side::Right)]
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
                .insert(KvId(i), KvId(i), &sha256_num(i), InsertLocation::Auto {})
                .unwrap();
            let end = Instant::now();
            total_time += end.duration_since(start);
        }

        println!("total time: {total_time:?}");
        // TODO: check, well...  something

        merkle_blob.calculate_lazy_hashes().unwrap();
    }

    #[test]
    fn test_delete_in_reverse_creates_matching_trees() {
        const COUNT: usize = 10;
        let mut dots = vec![];

        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();
        let mut reference_blobs = vec![];

        let key_value_ids: [KvId; COUNT] = core::array::from_fn(|i| KvId(i as i64));

        for key_value_id in key_value_ids {
            let hash: Hash = sha256_num(key_value_id.0);

            println!("inserting: {key_value_id}");
            merkle_blob.calculate_lazy_hashes().unwrap();
            reference_blobs.push(MerkleBlob::new(merkle_blob.blob.clone()).unwrap());
            merkle_blob
                .insert(key_value_id, key_value_id, &hash, InsertLocation::Auto {})
                .unwrap();
            dots.push(merkle_blob.to_dot().dump());
        }

        merkle_blob.check_integrity().unwrap();

        for key_value_id in key_value_ids.iter().rev() {
            println!("deleting: {key_value_id}");
            merkle_blob.delete(*key_value_id).unwrap();
            merkle_blob.calculate_lazy_hashes().unwrap();
            assert_eq!(merkle_blob, reference_blobs[key_value_id.0 as usize]);
            dots.push(merkle_blob.to_dot().dump());
        }
    }

    #[test]
    fn test_insert_first() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_id = KvId(1);
        open_dot(merkle_blob.to_dot().set_note("empty"));
        merkle_blob
            .insert(
                key_value_id,
                key_value_id,
                &sha256_num(key_value_id.0),
                InsertLocation::Auto {},
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().set_note("first after"));

        assert_eq!(merkle_blob.key_to_index.len(), 1);
    }

    #[rstest]
    fn test_insert_choosing_side(
        #[values(Side::Left, Side::Right)] side: Side,
        #[values(1, 2)] pre_count: usize,
    ) {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let mut last_key: KvId = KvId(0);
        for i in 1..=pre_count {
            let key = KvId(i as i64);
            open_dot(merkle_blob.to_dot().set_note("empty"));
            merkle_blob
                .insert(key, key, &sha256_num(key.0), InsertLocation::Auto {})
                .unwrap();
            last_key = key;
        }

        let key_value_id: KvId = KvId((pre_count + 1) as i64);
        open_dot(merkle_blob.to_dot().set_note("first after"));
        merkle_blob
            .insert(
                key_value_id,
                key_value_id,
                &sha256_num(key_value_id.0),
                InsertLocation::Leaf {
                    index: merkle_blob.key_to_index[&last_key],
                    side: side.clone(),
                },
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().set_note("first after"));

        let sibling = merkle_blob
            .get_node(merkle_blob.key_to_index[&last_key])
            .unwrap();
        let parent = merkle_blob.get_node(sibling.parent().unwrap()).unwrap();
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

        let expected_keys: [KvId; 2] = match side {
            Side::Left => [KvId(pre_count as i64 + 1), KvId(pre_count as i64)],
            Side::Right => [KvId(pre_count as i64), KvId(pre_count as i64 + 1)],
        };
        assert_eq!([left.key, right.key], expected_keys);
    }

    #[test]
    fn test_delete_last() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_id = KvId(1);
        open_dot(merkle_blob.to_dot().set_note("empty"));
        merkle_blob
            .insert(
                key_value_id,
                key_value_id,
                &sha256_num(key_value_id.0),
                InsertLocation::Auto {},
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().set_note("first after"));
        merkle_blob.check_integrity().unwrap();

        merkle_blob.delete(key_value_id).unwrap();

        assert_eq!(merkle_blob.key_to_index.len(), 0);
    }

    #[rstest]
    fn test_delete_frees_index(mut small_blob: MerkleBlob) {
        let key = KvId(0x0001_0203_0405_0607);
        let index = small_blob.key_to_index[&key];
        small_blob.delete(key).unwrap();

        assert_eq!(
            small_blob.free_indexes,
            HashSet::from([index, TreeIndex(2)])
        );
    }

    #[rstest]
    fn test_get_new_index_with_free_index(mut small_blob: MerkleBlob) {
        open_dot(small_blob.to_dot().set_note("initial"));
        let key = KvId(0x0001_0203_0405_0607);
        let _ = small_blob.key_to_index[&key];
        small_blob.delete(key).unwrap();
        open_dot(small_blob.to_dot().set_note("after delete"));

        let expected = HashSet::from([TreeIndex(1), TreeIndex(2)]);
        assert_eq!(small_blob.free_indexes, expected);
    }

    #[rstest]
    fn test_dump_small_blob_bytes(small_blob: MerkleBlob) {
        println!("{}", hex::encode(small_blob.blob.clone()));
    }

    #[test]
    fn test_node_type_from_u8_invalid() {
        let invalid_value = 2;
        let actual = NodeType::from_u8(invalid_value);
        actual.expect_err("invalid node type value should fail");
    }

    #[test]
    fn test_node_specific_sibling_index_panics_for_unknown_sibling() {
        let node = InternalNode {
            parent: None,
            hash: sha256_num(0),
            left: TreeIndex(0),
            right: TreeIndex(1),
        };
        let index = TreeIndex(2);
        assert_eq!(
            node.sibling_index(TreeIndex(2)),
            Err(Error::IndexIsNotAChild(index))
        );
    }

    #[rstest]
    fn test_get_free_indexes(small_blob: MerkleBlob) {
        let mut blob = small_blob.blob.clone();
        let expected_free_index = TreeIndex((blob.len() / BLOCK_SIZE) as u32);
        blob.extend_from_slice(&[0; BLOCK_SIZE]);
        let (free_indexes, _) = get_free_indexes_and_keys_values_indexes(&blob);
        assert_eq!(free_indexes, HashSet::from([expected_free_index]));
    }

    #[test]
    fn test_merkle_blob_new_errs_for_nonmultiple_of_block_length() {
        MerkleBlob::new(vec![1]).expect_err("invalid length should fail");
    }

    #[rstest]
    fn test_upsert_inserts(small_blob: MerkleBlob) {
        let key = KvId(1234);
        assert!(!small_blob.key_to_index.contains_key(&key));
        let value = KvId(5678);

        let mut insert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        insert_blob
            .insert(key, value, &sha256_num(key.0), InsertLocation::Auto {})
            .unwrap();
        open_dot(insert_blob.to_dot().set_note("first after"));

        let mut upsert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        upsert_blob.upsert(key, value, &sha256_num(key.0)).unwrap();
        open_dot(upsert_blob.to_dot().set_note("first after"));

        assert_eq!(insert_blob.blob, upsert_blob.blob);
    }

    #[rstest]
    fn test_upsert_upserts(mut small_blob: MerkleBlob) {
        let before_blocks =
            MerkleBlobLeftChildFirstIterator::new(&small_blob.blob).collect::<Vec<_>>();
        let (key, index) = small_blob.key_to_index.iter().next().unwrap();
        let original = small_blob.get_node(*index).unwrap().expect_leaf("<<self>>");
        let new_value = KvId(original.value.0 + 1);

        small_blob.upsert(*key, new_value, &original.hash).unwrap();

        let after_blocks =
            MerkleBlobLeftChildFirstIterator::new(&small_blob.blob).collect::<Vec<_>>();

        assert_eq!(before_blocks.len(), after_blocks.len());
        for ((before_index, before_block), (after_index, after_block)) in
            zip(before_blocks, after_blocks)
        {
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
        let kv = KvId(0);
        blob.insert(kv, kv, &Bytes32::new([0u8; 32]), InsertLocation::Auto {})
            .unwrap();
        blob.insert(kv, kv, &Bytes32::new([0u8; 32]), InsertLocation::Auto {})
            .expect_err("");
    }

    #[rstest]
    fn test_batch_insert(
        #[values(0, 1, 2, 10)] pre_inserts: usize,
        #[values(0, 1, 2, 8, 9)] count: usize,
    ) {
        let mut blob = MerkleBlob::new(vec![]).unwrap();
        for i in 0..pre_inserts {
            let i = KvId(i as i64);
            blob.insert(i, i, &sha256_num(i.0), InsertLocation::Auto {})
                .unwrap();
        }
        open_dot(blob.to_dot().set_note("initial"));

        let mut batch: Vec<((KvId, KvId), Hash)> = vec![];

        let mut batch_map = HashMap::new();
        for i in pre_inserts..(pre_inserts + count) {
            let i = KvId(i as i64);
            batch.push(((i, i), sha256_num(i.0)));
            batch_map.insert(i, i);
        }

        let before = blob.get_key_value_map();
        blob.batch_insert(batch.into_iter()).unwrap();
        let after = blob.get_key_value_map();

        open_dot(
            blob.to_dot()
                .set_note(&format!("after batch insert of {count} values")),
        );

        let mut expected = before.clone();
        expected.extend(batch_map);

        assert_eq!(after, expected);
    }
}
