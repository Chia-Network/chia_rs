use crate::merkle::error::Error;
use crate::{Side, block_range, internal_hash};
use chia_protocol::Bytes32;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};
use chia_streamable_macro::Streamable;
use chia_traits::Streamable;
#[cfg(feature = "py-bindings")]
use pyo3::{Bound, FromPyObject, IntoPyObject, PyAny, PyErr, Python, pyclass, pymethods};
use std::ops::Range;

pub type TreeIndexType = u32;

#[cfg_attr(
    feature = "py-bindings",
    pyclass(frozen),
    derive(PyJsonDict, PyStreamable)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
// ISSUE: this cfg()/cfg(not()) is terrible, but there's an issue with pyo3
//        being found with a cfg_attr
//        https://github.com/PyO3/pyo3/issues/5125
#[cfg(feature = "py-bindings")]
pub struct TreeIndex(#[pyo3(get, name = "raw")] pub TreeIndexType);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
#[cfg(not(feature = "py-bindings"))]
pub struct TreeIndex(pub TreeIndexType);

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
pub struct Parent(pub Option<TreeIndex>);

#[cfg_attr(
    feature = "py-bindings",
    derive(FromPyObject, IntoPyObject, PyJsonDict),
    pyo3(transparent)
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
pub struct Hash(pub Bytes32);

/// Key and value ids are provided from outside of this code and are implemented as
/// the row id from sqlite which is a signed 8 byte integer.  The actual key and
/// value data bytes will not be handled within this code, only outside.
#[cfg_attr(
    feature = "py-bindings",
    pyclass(frozen),
    derive(PyJsonDict, PyStreamable)
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
// ISSUE: this cfg()/cfg(not()) is terrible, but there's an issue with pyo3
//        being found with a cfg_attr
//        https://github.com/PyO3/pyo3/issues/5125
#[cfg(feature = "py-bindings")]
pub struct KeyId(#[pyo3(get, name = "raw")] pub i64);

#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
#[cfg(not(feature = "py-bindings"))]
pub struct KeyId(pub i64);

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
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
// ISSUE: this cfg()/cfg(not()) is terrible, but there's an issue with pyo3
//        being found with a cfg_attr
//        https://github.com/PyO3/pyo3/issues/5125
#[cfg(feature = "py-bindings")]
pub struct ValueId(#[pyo3(get, name = "raw")] pub i64);

#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Streamable)]
#[cfg(not(feature = "py-bindings"))]
pub struct ValueId(pub i64);

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

// define the serialized block format
const METADATA_RANGE: Range<usize> = 0..METADATA_SIZE;
pub const METADATA_SIZE: usize = 2;
// TODO: figure out the real max better than trial and error?
pub const DATA_SIZE: usize = 53;
pub const BLOCK_SIZE: usize = METADATA_SIZE + DATA_SIZE;

pub type BlockBytes = [u8; BLOCK_SIZE];
type MetadataBytes = [u8; METADATA_SIZE];
type DataBytes = [u8; DATA_SIZE];

const DATA_RANGE: Range<usize> = METADATA_SIZE..METADATA_SIZE + DATA_SIZE;

pub(crate) fn streamable_from_bytes_ignore_extra_bytes<T>(
    bytes: &[u8],
) -> Result<T, chia_traits::chia_error::Error>
where
    T: Streamable,
{
    let mut cursor = std::io::Cursor::new(bytes);
    T::parse::<false>(&mut cursor)
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Streamable)]
pub enum NodeType {
    Internal = 0,
    Leaf = 1,
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
    pub hash: Hash,
    pub parent: Parent,
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
    pub hash: Hash,
    pub parent: Parent,
    pub key: KeyId,
    pub value: ValueId,
}

// TODO: consider forcing ::new() with validity checks
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
            Node::Internal(node) => node.hash = hash,
            Node::Leaf(node) => node.hash = hash,
        }
    }

    pub fn from_bytes(
        metadata: &NodeMetadata,
        blob: &DataBytes,
    ) -> Result<Self, chia_traits::chia_error::Error> {
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

    pub fn expect_internal(&self, message: &str) -> InternalNode {
        let Node::Internal(internal) = self else {
            let message = message.replace("<<self>>", &format!("{self:?}"));
            panic!("{}", message)
        };

        *internal
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

// TODO: consider forcing ::new() with validity checks
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    // NOTE: metadata node type and node's type not verified for agreement
    pub metadata: NodeMetadata,
    pub node: Node,
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
        let metadata =
            NodeMetadata::from_bytes(&metadata_blob).map_err(Error::FailedLoadingMetadata)?;
        let node = Node::from_bytes(&metadata, &data_blob).map_err(Error::FailedLoadingNode)?;

        Ok(Block { metadata, node })
    }

    pub fn update_hash(&mut self, left: &Hash, right: &Hash) {
        self.node.set_hash(internal_hash(left, right));
        self.metadata.dirty = false;
    }
}

pub fn try_get_block(blob: &[u8], index: TreeIndex) -> Result<Block, Error> {
    let range = block_range(index);
    let block_bytes: BlockBytes = blob
        .get(range)
        .ok_or(Error::BlockIndexOutOfBounds(index))?
        .try_into()
        .expect("used block_range() so should be correct length");

    Block::from_bytes(block_bytes)
}
