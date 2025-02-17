#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};
#[cfg(feature = "py-bindings")]
use pyo3::{
    buffer::PyBuffer,
    exceptions::PyValueError,
    prelude::*,
    pyclass, pymethods,
    types::{PyDict, PyDictMethods, PyListMethods},
    Bound, FromPyObject, IntoPyObject, PyAny, PyErr, PyResult, Python,
};

use chia_protocol::Bytes32;
use chia_sha2::Sha256;
use chia_streamable_macro::Streamable;
use chia_traits::Streamable;
use num_traits::ToBytes;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::iter::zip;
use std::ops::Range;
use thiserror::Error;

type TreeIndexType = u32;
#[cfg_attr(
    feature = "py-bindings",
    pyclass(frozen),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
// TODO: this cfg()/cfg(not()) is terrible, but there's an issue with pyo3
//       being found with a cfg_attr
#[cfg(feature = "py-bindings")]
pub struct TreeIndex(#[pyo3(get, name = "raw")] TreeIndexType);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
#[cfg(not(feature = "py-bindings"))]
pub struct TreeIndex(TreeIndexType);

#[cfg(feature = "py-bindings")]
#[pymethods]
impl TreeIndex {
    #[new]
    pub fn py_new(raw: TreeIndexType) -> Self {
        Self(raw)
    }
}

impl std::fmt::Display for TreeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg_attr(
    feature = "py-bindings",
    derive(FromPyObject, IntoPyObject, PyJsonDict),
    pyo3(transparent)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct Parent(Option<TreeIndex>);

#[cfg_attr(
    feature = "py-bindings",
    derive(FromPyObject, IntoPyObject, PyJsonDict),
    pyo3(transparent)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct Hash(Bytes32);

/// Key and value ids are provided from outside of this code and are implemented as
/// the row id from sqlite which is a signed 8 byte integer.  The actual key and
/// value data bytes will not be handled within this code, only outside.
#[cfg_attr(
    feature = "py-bindings",
    pyclass(frozen),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
// TODO: this cfg()/cfg(not()) is terrible, but there's an issue with pyo3
//       being found with a cfg_attr
#[cfg(feature = "py-bindings")]
pub struct KeyId(#[pyo3(get, name = "raw")] i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
#[cfg(not(feature = "py-bindings"))]
pub struct KeyId(i64);

#[cfg(feature = "py-bindings")]
#[pymethods]
impl KeyId {
    #[new]
    pub fn py_new(raw: i64) -> Self {
        Self(raw)
    }
}

#[cfg_attr(
    feature = "py-bindings",
    pyclass(frozen),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
// TODO: this cfg()/cfg(not()) is terrible, but there's an issue with pyo3
//       being found with a cfg_attr
#[cfg(feature = "py-bindings")]
pub struct ValueId(#[pyo3(get, name = "raw")] i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
#[cfg(not(feature = "py-bindings"))]
pub struct ValueId(i64);

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ValueId {
    #[new]
    pub fn py_new(raw: i64) -> Self {
        Self(raw)
    }
}

impl std::fmt::Display for ValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// consider https://github.com/Chia-Network/chia_rs/pull/872 when altendky is less of a noob
macro_rules! create_errors {
    (
        $enum:ident,
        (
            $(
                (
                    $name:ident,
                    $python_name:ident,
                    $string:literal,
                    (
                        $(
                            $type_:path
                        ),
                        *
                    )
                )
            ),
            *
        )
    ) => {
        #[derive(Debug, Error, PartialEq, Eq)]
        pub enum $enum {
            $(
                #[error($string)]
                $name($($type_,)*),
            )*
        }

        #[cfg(feature = "py-bindings")]
        pub mod python_exceptions {
            use super::*;

            $(
                pyo3::create_exception!(chia_rs.chia_rs.datalayer, $python_name, pyo3::exceptions::PyException);
            )*

            pub fn add_to_module(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
                $(
                    module.add(stringify!($python_name), py.get_type::<$python_name>())?;
                )*

                Ok(())
            }
        }

        #[cfg(feature = "py-bindings")]
        impl From<Error> for pyo3::PyErr {
            fn from(err: Error) -> pyo3::PyErr {
                let message = err.to_string();
                match err {
                    $(
                        Error::$name(..) => python_exceptions::$python_name::new_err(message),
                    )*
                }
            }
        }
    }
}

create_errors!(
    Error,
    (
        // TODO: don't use String here
        (
            FailedLoadingMetadata,
            FailedLoadingMetadataError,
            "failed loading metadata: {0}",
            (String)
        ),
        // TODO: don't use String here
        (
            FailedLoadingNode,
            FailedLoadingNodeError,
            "failed loading node: {0}",
            (String)
        ),
        (
            InvalidBlobLength,
            InvalidBlobLengthError,
            "blob length must be a multiple of block count, found extra bytes: {0}",
            (usize)
        ),
        (
            KeyAlreadyPresent,
            KeyAlreadyPresentError,
            "key already present",
            ()
        ),
        (
            UnableToInsertAsRootOfNonEmptyTree,
            UnableToInsertAsRootOfNonEmptyTreeError,
            "requested insertion at root but tree not empty",
            ()
        ),
        (
            UnableToFindALeaf,
            UnableToFindALeafError,
            "unable to find a leaf",
            ()
        ),
        (UnknownKey, UnknownKeyError, "unknown key: {0:?}", (KeyId)),
        (
            IntegrityKeyNotInCache,
            IntegrityKeyNotInCacheError,
            "key not in key to index cache: {0:?}",
            (KeyId)
        ),
        (
            IntegrityKeyToIndexCacheIndex,
            IntegrityKeyToIndexCacheIndexError,
            "key to index cache for {0:?} should be {1:?} got: {2:?}",
            (KeyId, TreeIndex, TreeIndex)
        ),
        (
            IntegrityParentChildMismatch,
            IntegrityParentChildMismatchError,
            "parent and child relationship mismatched: {0:?}",
            (TreeIndex)
        ),
        (
            IntegrityKeyToIndexCacheLength,
            IntegrityKeyToIndexCacheLengthError,
            "found {0:?} leaves but key to index cache length is: {1}",
            (usize, usize)
        ),
        (
            IntegrityUnmatchedChildParentRelationships,
            IntegrityUnmatchedChildParentRelationshipsError,
            "unmatched parent -> child references found: {0}",
            (usize)
        ),
        (
            IntegrityTotalNodeCount,
            IntegrityTotalNodeCountError,
            "expected total node count {0:?} found: {1:?}",
            (TreeIndex, usize)
        ),
        (
            ZeroLengthSeedNotAllowed,
            ZeroLengthSeedNotAllowedError,
            "zero-length seed bytes not allowed",
            ()
        ),
        (
            NodeNotALeaf,
            NodeNotALeafError,
            "node not a leaf: {0:?}",
            (InternalNode)
        ),
        (
            Streaming,
            StreamingError,
            "from streamable: {0:?}",
            (chia_traits::chia_error::Error)
        ),
        (
            IndexIsNotAChild,
            IndexIsNotAChildError,
            "index not a child: {0}",
            (TreeIndex)
        ),
        (CycleFound, CycleFoundError, "cycle found", ()),
        (
            BlockIndexOutOfBounds,
            BlockIndexOutOfBoundsError,
            "block index out of bounds: {0}",
            (TreeIndex)
        )
    )
);

// assumptions
// - root is at index 0
// - any case with no keys will have a zero length blob

// define the serialized block format
const METADATA_RANGE: Range<usize> = 0..METADATA_SIZE;
pub const METADATA_SIZE: usize = 2;
// TODO: figure out the real max better than trial and error?
pub const DATA_SIZE: usize = 53;
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

#[cfg_attr(feature = "py-bindings", pyclass(get_all))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ProofOfInclusionLayer {
    pub other_hash_side: Side,
    pub other_hash: Hash,
    pub combined_hash: Hash,
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ProofOfInclusionLayer {
    #[new]
    pub fn py_init(other_hash_side: Side, other_hash: Hash, combined_hash: Hash) -> PyResult<Self> {
        Ok(Self {
            other_hash_side,
            other_hash,
            combined_hash,
        })
    }
}

#[cfg_attr(feature = "py-bindings", pyclass(get_all))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
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
            let calculated_hash =
                calculate_internal_hash(&existing_hash, layer.other_hash_side, &layer.other_hash);

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

#[allow(clippy::needless_pass_by_value)]
fn sha256_num<T: ToBytes>(input: T) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input.to_be_bytes());

    Hash(Bytes32::new(hasher.finalize()))
}

fn sha256_bytes(input: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input);

    Hash(Bytes32::new(hasher.finalize()))
}

fn internal_hash(left_hash: &Hash, right_hash: &Hash) -> Hash {
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

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq, Streamable)]
pub struct NodeMetadata {
    // OPT: could save 1-2% of tree space by packing (and maybe don't do that)
    pub node_type: NodeType,
    pub dirty: bool,
}

#[cfg_attr(
    feature = "py-bindings",
    pyclass(get_all),
    derive(PyJsonDict, PyStreamable)
)]
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

    pub fn get_sibling_side(&self, index: TreeIndex) -> Result<Side, Error> {
        if self.left == index {
            Ok(Side::Right)
        } else if self.right == index {
            Ok(Side::Left)
        } else {
            Err(Error::IndexIsNotAChild(index))
        }
    }
}

#[cfg_attr(
    feature = "py-bindings",
    pyclass(get_all),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Streamable)]
pub struct LeafNode {
    pub parent: Parent,
    pub hash: Hash,
    pub key: KeyId,
    pub value: ValueId,
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

    fn expect_internal(&self, message: &str) -> InternalNode {
        let Node::Internal(internal) = self else {
            let message = message.replace("<<self>>", &format!("{self:?}"));
            panic!("{}", message)
        };

        *internal
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
) -> Result<(HashSet<TreeIndex>, HashMap<KeyId, TreeIndex>), Error> {
    let index_count = blob.len() / BLOCK_SIZE;

    let mut seen_indexes: Vec<bool> = vec![false; index_count];
    let mut key_to_index: HashMap<KeyId, TreeIndex> = HashMap::default();

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
    key_to_index: HashMap<KeyId, TreeIndex>,
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
        key: KeyId,
        value: ValueId,
        hash: &Hash,
        insert_location: InsertLocation,
    ) -> Result<TreeIndex, Error> {
        if self.key_to_index.contains_key(&key) {
            return Err(Error::KeyAlreadyPresent());
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
                if !self.key_to_index.is_empty() {
                    return Err(Error::UnableToInsertAsRootOfNonEmptyTree());
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
                    parent: Parent(None),
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
        };

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

        if self.key_to_index.len() <= 1 {
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
            self.insert_from_key(min_height_leaf.key, indexes[0], Side::Left)?;
        };

        Ok(())
    }

    fn insert_from_key(
        &mut self,
        old_leaf_key: KeyId,
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

        // TODO: expect relates to comment at the beginning about assumptions about the tree etc
        let old_leaf_parent = old_leaf.parent.0.expect("not handling this case");

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
        let (_index, block) = MerkleBlobBreadthFirstIterator::new(&self.blob)
            .next()
            .ok_or(Error::UnableToFindALeaf())??;

        Ok(block
            .node
            .expect_leaf("unexpectedly found internal node first: <<self>>"))
    }

    pub fn delete(&mut self, key: KeyId) -> Result<(), Error> {
        let (leaf_index, leaf, _leaf_block) = self.get_leaf_by_key(key)?;
        self.key_to_index.remove(&key);

        let Some(parent_index) = leaf.parent.0 else {
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

        let Some(grandparent_index) = parent.parent.0 else {
            sibling_block.node.set_parent(Parent(None));
            if let Node::Internal(node) = sibling_block.node {
                sibling_block.metadata.dirty = true;
                for child_index in [node.left, node.right] {
                    self.update_parent(child_index, Some(TreeIndex(0)))?;
                }
            };

            self.insert_entry_to_blob(TreeIndex(0), &sibling_block)?;
            self.free_indexes.insert(sibling_index);

            return Ok(());
        };

        self.free_indexes.insert(parent_index);
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

        leaf.hash.clone_from(new_hash);
        leaf.value = value;
        // OPT: maybe just edit in place?
        block.node = Node::Leaf(leaf);
        self.insert_entry_to_blob(leaf_index, &block)?;

        if let Some(parent) = block.node.parent().0 {
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
        let side = if (seed_bytes.last().ok_or(Error::ZeroLengthSeedNotAllowed())? & 1 << 7) == 0 {
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

            seed_bytes = sha256_bytes(&seed_bytes).0.into();
        }
    }

    fn get_random_insert_location_by_key_id(&self, seed: KeyId) -> Result<InsertLocation, Error> {
        let seed = sha256_num(seed.0);

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
            .ok_or(Error::BlockIndexOutOfBounds(index))?
            .try_into()
            .unwrap_or_else(|e| panic!("failed getting block {index}: {e}")))
    }

    pub fn get_node(&self, index: TreeIndex) -> Result<Node, Error> {
        Ok(self.get_block(index)?.node)
    }

    pub fn get_leaf_by_key(&self, key: KeyId) -> Result<(TreeIndex, LeafNode, Block), Error> {
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
            next_index = node.parent().0;
            lineage.push((this_index, node));
        }

        Ok(lineage)
    }

    pub fn get_lineage_indexes(&self, index: TreeIndex) -> Result<Vec<TreeIndex>, Error> {
        let mut next_index = Some(index);
        let mut lineage: Vec<TreeIndex> = vec![];

        while let Some(this_index) = next_index {
            lineage.push(this_index);
            next_index = self.get_parent_index(this_index)?.0;
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

    pub fn get_keys_values(&self) -> Result<HashMap<KeyId, ValueId>, Error> {
        let mut map = HashMap::new();
        for (key, index) in &self.key_to_index {
            let node = self.get_node(*index)?;
            let leaf = node.expect_leaf(
                "key was just retrieved from the key to index mapping, must be a leaf",
            );
            map.insert(*key, leaf.value);
        }

        Ok(map)
    }

    pub fn get_key_index(&self, key: KeyId) -> Result<TreeIndex, Error> {
        self.key_to_index
            .get(&key)
            .copied()
            .ok_or(Error::UnknownKey(key))
    }

    pub fn get_proof_of_inclusion(&self, key: KeyId) -> Result<ProofOfInclusion, Error> {
        let mut index = *self.key_to_index.get(&key).ok_or(Error::UnknownKey(key))?;

        let node = self
            .get_node(index)?
            .expect_leaf("key to index mapping should only have leaves");

        let parents = self.get_lineage_with_indexes(index)?;
        let mut layers: Vec<ProofOfInclusionLayer> = Vec::new();
        let mut parents_iter = parents.iter();
        // first in the lineage is the index itself, second is the first parent
        parents_iter.next();
        for (next_index, parent) in parents_iter {
            let parent = parent.expect_internal("all nodes after the first should be internal");
            let sibling_index = parent.sibling_index(index)?;
            let sibling_block = self.get_block(sibling_index)?;
            assert!(!sibling_block.metadata.dirty);
            let sibling = sibling_block.node;
            let layer = ProofOfInclusionLayer {
                other_hash_side: parent.get_sibling_side(index)?,
                other_hash: sibling.hash(),
                combined_hash: parent.hash,
            };
            layers.push(layer);
            index = *next_index;
        }

        Ok(ProofOfInclusion {
            node_hash: node.hash,
            layers,
        })
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
                    .key_to_index
                    .get(&key)
                    // TODO: use a specific error
                    .ok_or(PyValueError::new_err(format!(
                        "unknown key id passed as insert location reference: {key}"
                    )))?,
                side: Side::from_bytes(&[side])?,
            },
            _ => {
                // TODO: use a specific error
                return Err(PyValueError::new_err(
                    "must specify neither or both of reference_kid and side",
                ));
            }
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
    pub fn py_get_lineage_with_indexes(
        &self,
        index: TreeIndex,
        py: Python<'_>,
    ) -> PyResult<pyo3::PyObject> {
        let list = pyo3::types::PyList::empty(py);

        for (index, node) in self.get_lineage_with_indexes(index)? {
            list.append((index.into_pyobject(py)?, node.into_pyobject(py)?))?;
        }

        Ok(list.into())
    }

    #[pyo3(name = "get_nodes_with_indexes")]
    pub fn py_get_nodes_with_indexes(&self, py: Python<'_>) -> PyResult<pyo3::PyObject> {
        let list = pyo3::types::PyList::empty(py);

        for item in MerkleBlobParentFirstIterator::new(&self.blob) {
            let (index, block) = item?;
            list.append((index.into_pyobject(py)?, block.node.into_pyobject(py)?))?;
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

        let block = self.get_block(index)?;
        if block.metadata.dirty {
            // TODO: use a specific error
            return Err(PyValueError::new_err("root hash is dirty"));
        }

        Ok(Some(block.node.hash()))
    }

    #[pyo3(name = "batch_insert")]
    pub fn py_batch_insert(
        &mut self,
        keys_values: Vec<(KeyId, ValueId)>,
        hashes: Vec<Hash>,
    ) -> PyResult<()> {
        if keys_values.len() != hashes.len() {
            // TODO: use a specific error
            return Err(PyValueError::new_err(
                "key/value and hash collection lengths must match",
            ));
        }

        self.batch_insert(zip(keys_values, hashes).collect())?;

        Ok(())
    }

    #[pyo3(name = "__len__")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.blob.len())
    }

    #[pyo3(name = "get_keys_values")]
    pub fn py_get_keys_values(&self, py: Python<'_>) -> PyResult<pyo3::PyObject> {
        let map = self.get_keys_values()?;
        let dict = PyDict::new(py);
        for (key, value) in map {
            dict.set_item(key, value)?;
        }

        Ok(dict.into())
    }

    #[pyo3(name = "get_key_index")]
    pub fn py_get_key_index(&self, key: KeyId) -> PyResult<TreeIndex> {
        Ok(self.get_key_index(key)?)
    }

    #[pyo3(name = "get_proof_of_inclusion")]
    pub fn py_get_proof_of_inclusion(&self, key: KeyId) -> PyResult<ProofOfInclusion> {
        Ok(self.get_proof_of_inclusion(key)?)
    }
}

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
                        return Some(Err(Error::CycleFound()));
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
                return Some(Err(Error::CycleFound()));
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
                        return Some(Err(Error::CycleFound()));
                    }
                    self.already_queued.insert(index);

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
        if self.check_integrity_on_drop {
            self.check_integrity()
                .expect("integrity check failed while dropping merkle blob");
        }
    }
}

#[cfg(test)]
mod dot;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::dot::DotLines;
    use expect_test::{expect, Expect};
    use rand::rngs::StdRng;
    use rand::seq::SliceRandom;
    use rand::SeedableRng;
    use rstest::{fixture, rstest};
    use std::time::{Duration, Instant};

    fn open_dot(_lines: &mut DotLines) {
        // crate::merkle::dot::open_dot(_lines);
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
                streamable_from_bytes_ignore_extra_bytes::<NodeType>(&[node_type as u8]).unwrap(),
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

    #[fixture]
    fn small_blob() -> MerkleBlob {
        let mut blob = MerkleBlob::new(vec![]).unwrap();

        blob.insert(
            KeyId(0x0001_0203_0405_0607),
            ValueId(0x1011_1213_1415_1617),
            &sha256_num(0x1020),
            InsertLocation::Auto {},
        )
        .unwrap();

        blob.insert(
            KeyId(0x2021_2223_2425_2627),
            ValueId(0x3031_3233_3435_3637),
            &sha256_num(0x2030),
            InsertLocation::Auto {},
        )
        .unwrap();

        blob
    }

    #[fixture]
    fn traversal_blob(mut small_blob: MerkleBlob) -> MerkleBlob {
        small_blob
            .insert(
                KeyId(103),
                ValueId(204),
                &sha256_num(0x1324),
                InsertLocation::Leaf {
                    index: TreeIndex(1),
                    side: Side::Right,
                },
            )
            .unwrap();
        small_blob
            .insert(
                KeyId(307),
                ValueId(404),
                &sha256_num(0x9183),
                InsertLocation::Leaf {
                    index: TreeIndex(3),
                    side: Side::Right,
                },
            )
            .unwrap();

        small_blob
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

    #[test]
    fn test_get_random_insert_location_by_seed_with_seed_too_short() {
        let mut blob = MerkleBlob::new(vec![]).unwrap();
        let seed = [0xff];
        let layer_count = 8 * seed.len() + 10;

        for n in 0..layer_count {
            let n = (n + 100) as i64;
            let key = KeyId(n);
            let value = ValueId(n);
            let hash = sha256_num(key.0);
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
                    &sha256_num(i),
                    InsertLocation::Auto {},
                )
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

        let key_value_ids: [i64; COUNT] = core::array::from_fn(|i| i as i64);

        for key_value_id in key_value_ids {
            let hash: Hash = sha256_num(key_value_id);

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
            assert_eq!(merkle_blob, reference_blobs[*key_value_id as usize]);
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
                &sha256_num(key_value_id),
                InsertLocation::Auto {},
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));

        assert_eq!(merkle_blob.key_to_index.len(), 1);
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
                    &sha256_num(key_value),
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
                &sha256_num(key_value_id),
                InsertLocation::Leaf {
                    index: merkle_blob.key_to_index[&last_key],
                    side,
                },
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));

        let sibling = merkle_blob
            .get_node(merkle_blob.key_to_index[&last_key])
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
                &sha256_num(key_value_id),
                InsertLocation::Auto {},
            )
            .unwrap();
        open_dot(merkle_blob.to_dot().unwrap().set_note("first after"));
        merkle_blob.check_integrity().unwrap();

        merkle_blob.delete(KeyId(key_value_id)).unwrap();

        assert_eq!(merkle_blob.key_to_index.len(), 0);
    }

    #[rstest]
    fn test_delete_frees_index(mut small_blob: MerkleBlob) {
        let key = KeyId(0x0001_0203_0405_0607);
        let index = small_blob.key_to_index[&key];
        small_blob.delete(key).unwrap();

        assert_eq!(
            small_blob.free_indexes,
            HashSet::from([index, TreeIndex(2)])
        );
    }

    #[rstest]
    fn test_get_new_index_with_free_index(mut small_blob: MerkleBlob) {
        open_dot(small_blob.to_dot().unwrap().set_note("initial"));
        let key = KeyId(0x0001_0203_0405_0607);
        let _ = small_blob.key_to_index[&key];
        small_blob.delete(key).unwrap();
        open_dot(small_blob.to_dot().unwrap().set_note("after delete"));

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
        let actual = streamable_from_bytes_ignore_extra_bytes::<NodeType>(&[invalid_value as u8]);
        actual.expect_err("invalid node type value should fail");
    }

    #[test]
    fn test_node_specific_sibling_index_panics_for_unknown_sibling() {
        let node = InternalNode {
            parent: Parent(None),
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
        let (free_indexes, _) = get_free_indexes_and_keys_values_indexes(&blob).unwrap();
        assert_eq!(free_indexes, HashSet::from([expected_free_index]));
    }

    #[test]
    fn test_merkle_blob_new_errs_for_nonmultiple_of_block_length() {
        MerkleBlob::new(vec![1]).expect_err("invalid length should fail");
    }

    #[rstest]
    fn test_upsert_inserts(small_blob: MerkleBlob) {
        let key = KeyId(1234);
        assert!(!small_blob.key_to_index.contains_key(&key));
        let value = ValueId(5678);

        let mut insert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        insert_blob
            .insert(key, value, &sha256_num(key.0), InsertLocation::Auto {})
            .unwrap();
        open_dot(insert_blob.to_dot().unwrap().set_note("first after"));

        let mut upsert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        upsert_blob.upsert(key, value, &sha256_num(key.0)).unwrap();
        open_dot(upsert_blob.to_dot().unwrap().set_note("first after"));

        assert_eq!(insert_blob.blob, upsert_blob.blob);
    }

    #[rstest]
    fn test_upsert_upserts(mut small_blob: MerkleBlob) {
        let before_blocks =
            MerkleBlobLeftChildFirstIterator::new(&small_blob.blob).collect::<Vec<_>>();
        let (key, index) = small_blob.key_to_index.iter().next().unwrap();
        let original = small_blob.get_node(*index).unwrap().expect_leaf("<<self>>");
        let new_value = ValueId(original.value.0 + 1);

        small_blob.upsert(*key, new_value, &original.hash).unwrap();

        let after_blocks =
            MerkleBlobLeftChildFirstIterator::new(&small_blob.blob).collect::<Vec<_>>();

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
                &sha256_num(i),
                InsertLocation::Auto {},
            )
            .unwrap();
        }
        open_dot(blob.to_dot().unwrap().set_note("initial"));

        let mut batch: Vec<((KeyId, ValueId), Hash)> = vec![];

        let mut batch_map: HashMap<KeyId, ValueId> = HashMap::new();
        for i in pre_inserts..(pre_inserts + count) {
            let i = i as i64;
            batch.push(((KeyId(i), ValueId(i)), sha256_num(i)));
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

    fn iterator_test_reference(index: TreeIndex, block: &Block) -> (u32, NodeType, i64, i64, Hash) {
        match block.node {
            Node::Leaf(leaf) => (
                index.0,
                block.metadata.node_type,
                leaf.key.0,
                leaf.value.0,
                block.node.hash(),
            ),
            Node::Internal(internal) => (
                index.0,
                block.metadata.node_type,
                internal.left.0 as i64,
                internal.right.0 as i64,
                block.node.hash(),
            ),
        }
    }

    #[rstest]
    // expect-test is adding them back
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::left_child_first(
        "left child first",
        MerkleBlobLeftChildFirstIterator::new,
        expect![[r#"
            [
                (
                    1,
                    Leaf,
                    283686952306183,
                    1157726452361532951,
                    Hash(
                        d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
                (
                    6,
                    Internal,
                    3,
                    5,
                    Hash(
                        b946284149e4f4a0e767ef2feb397533fb112bf4d99c887348cec4438e38c1ce,
                    ),
                ),
                (
                    4,
                    Internal,
                    1,
                    6,
                    Hash(
                        eee0c40977ba1c0e16a467f30f64d9c2579ff25dd01913e33962c3f1db86c2ea,
                    ),
                ),
                (
                    2,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    0,
                    Internal,
                    4,
                    2,
                    Hash(
                        0e4a8b1ecee43f457bbe2b30e94ac2afc0d3a6536f891a2ced5e96ce07fe9932,
                    ),
                ),
            ]
        "#]],
    )]
    // expect-test is adding them back
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::parent_first(
        "parent first",
        MerkleBlobParentFirstIterator::new,
        expect![[r#"
            [
                (
                    0,
                    Internal,
                    4,
                    2,
                    Hash(
                        0e4a8b1ecee43f457bbe2b30e94ac2afc0d3a6536f891a2ced5e96ce07fe9932,
                    ),
                ),
                (
                    4,
                    Internal,
                    1,
                    6,
                    Hash(
                        eee0c40977ba1c0e16a467f30f64d9c2579ff25dd01913e33962c3f1db86c2ea,
                    ),
                ),
                (
                    2,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    1,
                    Leaf,
                    283686952306183,
                    1157726452361532951,
                    Hash(
                        d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d,
                    ),
                ),
                (
                    6,
                    Internal,
                    3,
                    5,
                    Hash(
                        b946284149e4f4a0e767ef2feb397533fb112bf4d99c887348cec4438e38c1ce,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
            ]
        "#]])]
    // expect-test is adding them back
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::breadth_first(
        "breadth first",
        MerkleBlobBreadthFirstIterator::new,
        expect![[r#"
            [
                (
                    2,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    1,
                    Leaf,
                    283686952306183,
                    1157726452361532951,
                    Hash(
                        d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
            ]
        "#]])]
    fn test_iterators<'a, F, T>(
        #[case] note: &str,
        #[case] iterator_new: F,
        #[case] expected: Expect,
        #[by_ref] traversal_blob: &'a MerkleBlob,
    ) where
        F: Fn(&'a Vec<u8>) -> T,
        T: Iterator<Item = Result<(TreeIndex, Block), Error>>,
    {
        let mut dot_actual = traversal_blob.to_dot().unwrap();
        dot_actual.set_note(note);

        let mut actual = vec![];
        {
            let blob: &Vec<u8> = &traversal_blob.blob;
            for item in iterator_new(blob) {
                let (index, block) = item.unwrap();
                actual.push(iterator_test_reference(index, &block));
                dot_actual.push_traversal(index);
            }
        }

        traversal_blob.to_dot().unwrap();

        open_dot(&mut dot_actual);

        expected.assert_debug_eq(&actual);
    }

    #[rstest]
    fn test_root_insert_location_when_not_empty(mut small_blob: MerkleBlob) {
        small_blob
            .insert(
                KeyId(0),
                ValueId(0),
                &sha256_num(0),
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
                    &sha256_num(n),
                    InsertLocation::Auto {},
                )
                .unwrap();
        }
        let (key, index) = {
            let (key, index) = small_blob.key_to_index.iter().next().unwrap();
            (*key, *index)
        };
        let expected_length = small_blob.blob.len();
        assert!(!small_blob.free_indexes.contains(&index));
        small_blob.delete(key).unwrap();
        assert!(small_blob.free_indexes.contains(&index));
        let free_indexes = small_blob.free_indexes.clone();
        assert_eq!(free_indexes.len(), 2);
        let new_index = small_blob
            .insert(
                KeyId(count),
                ValueId(count),
                &sha256_num(count),
                InsertLocation::Auto {},
            )
            .unwrap();
        assert_eq!(small_blob.blob.len(), expected_length);
        assert!(free_indexes.contains(&new_index));
        assert!(small_blob.free_indexes.is_empty());
    }

    fn generate_kvid(seed: i64) -> (KeyId, ValueId) {
        let mut kv_ids: Vec<i64> = Vec::new();

        for offset in 0..2 {
            let seed_int = 2i64 * seed + offset;
            let seed_bytes = seed_int.to_be_bytes();
            let hash = sha256_bytes(&seed_bytes);
            let hash_int = i64::from_be_bytes(hash.0[0..8].try_into().unwrap());
            kv_ids.push(hash_int);
        }

        (KeyId(kv_ids[0]), ValueId(kv_ids[1]))
    }

    fn generate_hash(seed: i64) -> Hash {
        let seed_bytes = seed.to_be_bytes();
        sha256_bytes(&seed_bytes)
    }

    #[test]
    fn test_proof_of_inclusion() {
        let num_repeats = 2; //10;
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
            for i in 0..(merkle_blob.blob.len() / BLOCK_SIZE) {
                let node = merkle_blob.get_node(TreeIndex(i as u32)).unwrap();
                println!("{i:05}: {node:?}");
            }

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
}
