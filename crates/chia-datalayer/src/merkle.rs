#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, Bound, FromPyObject, IntoPyObject, PyAny, PyErr, Python};

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

#[cfg_attr(
    feature = "py-bindings",
    derive(FromPyObject),
    derive(IntoPyObject),
    pyo3(transparent)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct TreeIndex(u32);

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
#[cfg_attr(
    feature = "py-bindings",
    derive(FromPyObject),
    derive(IntoPyObject),
    pyo3(transparent)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct KvId(i64);

impl std::fmt::Display for KvId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
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

    #[error("key to index cache for {0:?} should be {1:?} got: {2:?}")]
    IntegrityKeyToIndexCacheIndex(KvId, TreeIndex, TreeIndex),

    #[error("parent and child relationship mismatched: {0:?}")]
    IntegrityParentChildMismatch(TreeIndex),

    #[error("found {0:?} leaves but key to index cache length is: {1}")]
    IntegrityKeyToIndexCacheLength(usize, usize),

    #[error("unmatched parent -> child references found: {0}")]
    IntegrityUnmatchedChildParentRelationships(usize),

    #[error("expected total node count {0:?} found: {1:?}")]
    IntegrityTotalNodeCount(TreeIndex, usize),

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

    #[error("cycle found")]
    CycleFound,

    #[error("block index out of bounds: {0}")]
    BlockIndexOutOfBounds(TreeIndex),
}

// assumptions
// - root is at index 0
// - any case with no keys will have a zero length blob

// define the serialized block format
const METADATA_RANGE: Range<usize> = 0..METADATA_SIZE;
const METADATA_SIZE: usize = 2;
// TODO: figure out the real max better than trial and error?
const DATA_SIZE: usize = 53;
pub const BLOCK_SIZE: usize = METADATA_SIZE + DATA_SIZE;
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

#[cfg_attr(feature = "py-bindings", pyclass(eq, eq_int))]
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

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, Streamable)]
pub struct NodeMetadata {
    // OPT: could save 1-2% of tree space by packing (and maybe don't do that)
    pub node_type: NodeType,
    pub dirty: bool,
}

#[cfg_attr(feature = "py-bindings", pyclass(get_all))]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Streamable)]
pub struct InternalNode {
    pub parent: Parent,
    pub hash: Hash,
    pub left: TreeIndex,
    pub right: TreeIndex,
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

#[cfg_attr(feature = "py-bindings", pyclass(get_all))]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Streamable)]
pub struct LeafNode {
    pub parent: Parent,
    pub hash: Hash,
    pub key: KvId,
    pub value: KvId,
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

    fn set_hash(&mut self, hash: Hash) {
        match self {
            Node::Internal(ref mut node) => node.hash = hash,
            Node::Leaf(ref mut node) => node.hash = hash,
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
impl<'py> IntoPyObject<'py> for Node {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self {
            Node::Internal(node) => Ok(node.into_pyobject(py)?.into_any()),
            Node::Leaf(node) => Ok(node.into_pyobject(py)?.into_any()),
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
        self.node.set_hash(internal_hash(left, right));
        self.metadata.dirty = false;
    }
}

fn get_free_indexes_and_keys_values_indexes(
    blob: &Vec<u8>,
) -> Result<(HashSet<TreeIndex>, HashMap<KvId, TreeIndex>), Error> {
    let index_count = blob.len() / BLOCK_SIZE;

    let mut seen_indexes: Vec<bool> = vec![false; index_count];
    let mut key_to_index: HashMap<KvId, TreeIndex> = HashMap::default();

    for item in MerkleBlobLeftChildFirstIterator::new(blob) {
        let (index, block) = item?;
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

    Ok((free_indexes, key_to_index))
}

/// Stores a DataLayer merkle tree in bytes and provides serialization on each access so that only
/// the parts presently in use are stored in active objects.  The bytes are grouped as blocks of
/// equal size regardless of being internal vs. external nodes so that block indexes can be used
/// for references to particular nodes and readily converted to byte indexes.  The leaf nodes
/// do not hold the DataLayer key and value data but instead an id for each of the key and value
/// such that the code using a merkle blob can store the key and value as they see fit.  Each node
/// stores the hash for the merkle aspect of the tree.
#[cfg_attr(feature = "py-bindings", pyclass(get_all))]
#[derive(Debug)]
pub struct MerkleBlob {
    blob: Vec<u8>,
    // TODO: would be nice for this to be deterministic ala a fifo set
    free_indexes: HashSet<TreeIndex>,
    key_to_index: HashMap<KvId, TreeIndex>,
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

        // TODO: maybe integrate integrity check here if quick enough
        let (free_indexes, key_to_index) = get_free_indexes_and_keys_values_indexes(&blob)?;

        let self_ = Self {
            blob,
            free_indexes,
            key_to_index,
            check_integrity_on_drop: true,
        };

        Ok(self_)
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
    ) -> Result<TreeIndex, Error> {
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
                self.insert_first(key, value, hash)
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
                    self.insert_second(node, &old_leaf, &internal_node_hash, side)
                } else {
                    self.insert_third_or_later(node, &old_leaf, index, &internal_node_hash, side)
                }
            }
        }
    }

    fn insert_first(&mut self, key: KvId, value: KvId, hash: &Hash) -> Result<TreeIndex, Error> {
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

        Ok(new_leaf_index)
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
            self.insert_from_key(min_height_leaf.key, indexes[0], Side::Left)?;
        };

        Ok(())
    }

    fn insert_from_key(
        &mut self,
        old_leaf_key: KvId,
        new_index: TreeIndex,
        side: Side,
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
        let (_index, block) = MerkleBlobBreadthFirstIterator::new(&self.blob)
            .next()
            .ok_or(Error::UnableToFindALeaf)??;

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
        };

        Ok(())
    }

    pub fn check_integrity(&self) -> Result<(), Error> {
        let mut leaf_count: usize = 0;
        let mut internal_count: usize = 0;
        let mut child_to_parent: HashMap<TreeIndex, TreeIndex> = HashMap::new();

        for item in MerkleBlobParentFirstIterator::new(&self.blob) {
            let (index, block) = item?;
            if let Some(parent) = block.node.parent() {
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
                        .key_to_index
                        .get(&node.key)
                        .ok_or(Error::IntegrityKeyNotInCache(node.key))?;
                    if *cached_index != index {
                        return Err(Error::IntegrityKeyToIndexCacheIndex(
                            node.key,
                            index,
                            *cached_index,
                        ));
                    };
                    assert!(
                        !self.free_indexes.contains(&index),
                        "{}",
                        format!("active index found in free index list: {index:?}")
                    );
                }
            }
        }

        let key_to_index_cache_length = self.key_to_index.len();
        if leaf_count != key_to_index_cache_length {
            return Err(Error::IntegrityKeyToIndexCacheLength(
                leaf_count,
                key_to_index_cache_length,
            ));
        }
        let total_count = leaf_count + internal_count + self.free_indexes.len();
        let extend_index = self.extend_index();
        if total_count != extend_index.0 as usize {
            return Err(Error::IntegrityTotalNodeCount(extend_index, total_count));
        };
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

    // TODO: not really that random
    fn get_random_insert_location_by_seed(
        &self,
        seed_bytes: &[u8],
    ) -> Result<InsertLocation, Error> {
        let mut seed_bytes = Vec::from(seed_bytes);

        if self.blob.is_empty() {
            return Ok(InsertLocation::AsRoot {});
        }

        // TODO: zero means left here but right below?
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
        // OPT: yeah, storing the whole set of blocks via collect is not great
        for item in MerkleBlobLeftChildFirstIterator::new(&self.blob).collect::<Vec<_>>() {
            let (index, mut block) = item?;
            // OPT: really want a pruned traversal, not filter
            if !block.metadata.dirty {
                continue;
            }

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
        for item in zip(
            MerkleBlobLeftChildFirstIterator::new(&self.blob),
            MerkleBlobLeftChildFirstIterator::new(&other.blob),
        ) {
            let (Ok((_, self_block)), Ok((_, other_block))) = item else {
                // TODO: it's an error though, hmm
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
}

// impl<'a> IntoIterator for &'a MerkleBlob {
//     type Item = (TreeIndex, Block);
//     type IntoIter = MerkleBlobLeftChildFirstIterator<'a>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         MerkleBlobLeftChildFirstIterator::new(&self.blob)
//     }
// }

fn try_get_block(blob: &[u8], index: TreeIndex) -> Result<Block, Error> {
    // TODO: check limits and return error
    let range = block_range(index);
    let block_bytes: BlockBytes = blob
        .get(range)
        .ok_or(Error::BlockIndexOutOfBounds(index))?
        .try_into()
        .unwrap();

    Block::from_bytes(block_bytes)
}

struct MerkleBlobLeftChildFirstIteratorItem {
    visited: bool,
    index: TreeIndex,
}

pub struct MerkleBlobLeftChildFirstIterator<'a> {
    blob: &'a Vec<u8>,
    deque: VecDeque<MerkleBlobLeftChildFirstIteratorItem>,
    already_queued: HashSet<TreeIndex>,
}

impl<'a> MerkleBlobLeftChildFirstIterator<'a> {
    fn new(blob: &'a Vec<u8>) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(MerkleBlobLeftChildFirstIteratorItem {
                visited: false,
                index: TreeIndex(0),
            });
        }

        Self {
            blob,
            deque,
            already_queued: HashSet::new(),
        }
    }
}

impl Iterator for MerkleBlobLeftChildFirstIterator<'_> {
    type Item = Result<(TreeIndex, Block), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, children before parents

        loop {
            let item = self.deque.pop_front()?;
            let block = match try_get_block(self.blob, item.index) {
                Ok(block) => block,
                Err(e) => return Some(Err(e)),
            };

            match block.node {
                Node::Leaf(..) => return Some(Ok((item.index, block))),
                Node::Internal(ref node) => {
                    if item.visited {
                        return Some(Ok((item.index, block)));
                    };

                    if self.already_queued.contains(&item.index) {
                        return Some(Err(Error::CycleFound));
                    }
                    self.already_queued.insert(item.index);

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
    blob: &'a Vec<u8>,
    deque: VecDeque<TreeIndex>,
    already_queued: HashSet<TreeIndex>,
}

impl<'a> MerkleBlobParentFirstIterator<'a> {
    fn new(blob: &'a Vec<u8>) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(TreeIndex(0));
        }

        Self {
            blob,
            deque,
            already_queued: HashSet::new(),
        }
    }
}

impl Iterator for MerkleBlobParentFirstIterator<'_> {
    type Item = Result<(TreeIndex, Block), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parents before children

        let index = self.deque.pop_front()?;
        let block = match try_get_block(self.blob, index) {
            Ok(block) => block,
            Err(e) => return Some(Err(e)),
        };

        if let Node::Internal(ref node) = block.node {
            if self.already_queued.contains(&index) {
                return Some(Err(Error::CycleFound));
            }
            self.already_queued.insert(index);

            self.deque.push_back(node.left);
            self.deque.push_back(node.right);
        }

        Some(Ok((index, block)))
    }
}

pub struct MerkleBlobBreadthFirstIterator<'a> {
    blob: &'a Vec<u8>,
    deque: VecDeque<TreeIndex>,
    already_queued: HashSet<TreeIndex>,
}

impl<'a> MerkleBlobBreadthFirstIterator<'a> {
    #[allow(unused)]
    fn new(blob: &'a Vec<u8>) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(TreeIndex(0));
        }

        Self {
            blob,
            deque,
            already_queued: HashSet::new(),
        }
    }
}

impl Iterator for MerkleBlobBreadthFirstIterator<'_> {
    type Item = Result<(TreeIndex, Block), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parent depth before child depth

        loop {
            let index = self.deque.pop_front()?;
            let block = match try_get_block(self.blob, index) {
                Ok(block) => block,
                Err(e) => return Some(Err(e)),
            };

            match block.node {
                Node::Leaf(..) => return Some(Ok((index, block))),
                Node::Internal(node) => {
                    if self.already_queued.contains(&index) {
                        return Some(Err(Error::CycleFound));
                    }
                    self.already_queued.insert(index);

                    self.deque.push_back(node.left);
                    self.deque.push_back(node.right);
                }
            }
        }
    }
}
