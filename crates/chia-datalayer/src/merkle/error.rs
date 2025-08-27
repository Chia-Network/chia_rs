use crate::{Hash, InternalNode, KeyId, TreeIndex};
use thiserror::Error;

#[cfg_attr(feature = "py-bindings", derive(chia_datalayer_macro::PythonError))]
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed loading metadata: {0}")]
    FailedLoadingMetadata(chia_traits::chia_error::Error),
    #[error("failed loading node: {0}")]
    FailedLoadingNode(chia_traits::chia_error::Error),
    #[error("blob length must be a multiple of block count, found extra bytes: {0}")]
    InvalidBlobLength(usize),
    #[error("key already present")]
    KeyAlreadyPresent(),
    #[error("hash already present")]
    HashAlreadyPresent(),
    #[error("requested insertion at root but tree not empty")]
    UnableToInsertAsRootOfNonEmptyTree(),
    #[error("unable to find a leaf")]
    UnableToFindALeaf(),
    #[error("unknown key: {0:?}")]
    UnknownKey(KeyId),
    #[error("key not in key to index cache: {0:?}")]
    IntegrityKeyNotInCache(KeyId),
    #[error("key to index cache for {0:?} should be {1:?} got: {2:?}")]
    IntegrityKeyToIndexCacheIndex(KeyId, TreeIndex, TreeIndex),
    #[error("parent and child relationship mismatched: {0:?}")]
    IntegrityParentChildMismatch(TreeIndex),
    #[error("found {0:?} leaves but key to index cache length is: {1}")]
    IntegrityKeyToIndexCacheLength(usize, usize),
    #[error("found {0:?} leaves but leaf hash to index cache length is: {1}")]
    IntegrityLeafHashToIndexCacheLength(usize, usize),
    #[error("unmatched parent -> child references found: {0}")]
    IntegrityUnmatchedChildParentRelationships(usize),
    #[error("expected total node count {0:?} found: {1:?}")]
    IntegrityTotalNodeCount(TreeIndex, usize),
    #[error("zero-length seed bytes not allowed")]
    ZeroLengthSeedNotAllowed(),
    #[error("node not a leaf: {0:?}")]
    NodeNotALeaf(InternalNode),
    #[error("from streamable: {0:?}")]
    Streaming(chia_traits::chia_error::Error),
    #[error("index not a child: {0}")]
    IndexIsNotAChild(TreeIndex),
    #[error("cycle found")]
    CycleFound(),
    #[error("block index out of bounds: {0}")]
    BlockIndexOutOfBounds(TreeIndex),
    #[error("leaf hash not found: {0:?}")]
    LeafHashNotFound(Hash),
    #[error("root hash and node list disagreement")]
    RootHashAndNodeListDisagreement(),
    #[error("node hash not in nodes: {0:?}")]
    NodeHashNotInNodeMaps(Hash),
    #[error("move source index not in use: {0:?}")]
    MoveSourceIndexNotInUse(TreeIndex),
    #[error("move destination index not in use: {0:?}")]
    MoveDestinationIndexNotInUse(TreeIndex),
    #[error("must specify neither or both of reference_kid and side")]
    IncompleteInsertLocationParameters(),
    #[error("hash is dirty for index: {0:?}")]
    Dirty(TreeIndex),
    #[error("hash is dirty for leaf index: {0:?}")]
    DirtyLeaf(TreeIndex),
    #[error("key/value and hash collection lengths must match: {0:?} keys/values, {0:?} hashes")]
    UnmatchedKeysAndValues(usize, usize),
    #[error("hash not found: {0:?}")]
    HashNotFound(Hash),
    #[error("reference to unknown parent")]
    ReferenceToUnknownParent(),
    #[error("root has parent")]
    RootHasParent(),
    #[error("unexpected parentless node")]
    UnexpectedParentlessNode(),
    #[error("child's parent disclaims the child")]
    ParentDisagreesWithChild(),
    #[error("leaf cannot be parent")]
    LeafCannotBeParent(),
    #[error("invalid children")]
    InvalidChildren(),
    #[error("insert subtree requires reference leaf is not root")]
    LeafCannotBeRootWhenInsertingSubtree(),
}
