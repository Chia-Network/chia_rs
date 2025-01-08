#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, Bound, FromPyObject, IntoPyObject, PyAny, PyErr, Python};

use chia_protocol::Bytes32;
use chia_streamable_macro::Streamable;
use chia_traits::Streamable;
use clvmr::sha2::Sha256;
use num_traits::ToBytes;
use std::collections::{HashMap, HashSet, VecDeque};
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
pub fn sha256_num<T: ToBytes>(input: T) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input.to_be_bytes());

    Bytes32::new(hasher.finalize())
}

pub fn sha256_bytes(input: &[u8]) -> Hash {
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
    pub fn parent(&self) -> Parent {
        match self {
            Node::Internal(node) => node.parent,
            Node::Leaf(node) => node.parent,
        }
    }

    pub fn set_parent(&mut self, parent: Parent) {
        match self {
            Node::Internal(node) => node.parent = parent,
            Node::Leaf(node) => node.parent = parent,
        }
    }

    pub fn hash(&self) -> Hash {
        match self {
            Node::Internal(node) => node.hash,
            Node::Leaf(node) => node.hash,
        }
    }

    pub fn set_hash(&mut self, hash: Hash) {
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

    pub fn expect_leaf(&self, message: &str) -> LeafNode {
        let Node::Leaf(leaf) = self else {
            let message = message.replace("<<self>>", &format!("{self:?}"));
            panic!("{}", message)
        };

        *leaf
    }

    pub fn try_into_leaf(self) -> Result<LeafNode, Error> {
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

pub fn get_free_indexes_and_keys_values_indexes(
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
#[cfg_attr(feature = "py-bindings", pyclass)]
#[derive(Debug)]
pub struct MerkleBlob {
    pub blob: Vec<u8>,
    // TODO: would be nice for this to be deterministic ala a fifo set
    pub free_indexes: HashSet<TreeIndex>,
    pub key_to_index: HashMap<KvId, TreeIndex>,
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
    pub fn new(blob: &'a Vec<u8>) -> Self {
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
    pub fn new(blob: &'a Vec<u8>) -> Self {
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
