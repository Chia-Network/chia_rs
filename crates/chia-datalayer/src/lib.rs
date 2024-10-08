#[cfg(feature = "py-bindings")]
use pyo3::{buffer::PyBuffer, pyclass, pymethods, PyResult};

use clvmr::sha2::Sha256;
use dot::DotLines;
use num_traits::ToBytes;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::iter::{zip, IntoIterator};
use std::mem::size_of;
use std::ops::Range;

mod dot;

type TreeIndex = u32;
type Parent = Option<TreeIndex>;
type Hash = [u8; 32];
type KvId = i64;

const fn range_by_length(start: usize, length: usize) -> Range<usize> {
    start..start + length
}

// define the serialized block format
// TODO: consider in more detail other serialization tools such as serde and streamable
// common fields
// TODO: better way to pick the max of key value and right range, until we move hash first
// TODO: clearly shouldn't be hard coded
const METADATA_SIZE: usize = 2;
const METADATA_RANGE: Range<usize> = 0..METADATA_SIZE;
const HASH_RANGE: Range<usize> = range_by_length(0, size_of::<Hash>());
// const PARENT_RANGE: Range<usize> = range_by_length(HASH_RANGE.end, size_of::<TreeIndex>());
const PARENT_RANGE: Range<usize> = HASH_RANGE.end..(HASH_RANGE.end + size_of::<TreeIndex>());
// internal specific fields
const LEFT_RANGE: Range<usize> = range_by_length(PARENT_RANGE.end, size_of::<TreeIndex>());
const RIGHT_RANGE: Range<usize> = range_by_length(LEFT_RANGE.end, size_of::<TreeIndex>());
// leaf specific fields
const KEY_RANGE: Range<usize> = range_by_length(PARENT_RANGE.end, size_of::<KvId>());
const VALUE_RANGE: Range<usize> = range_by_length(KEY_RANGE.end, size_of::<KvId>());

// TODO: clearly shouldn't be hard coded
// TODO: max of RIGHT_RANGE.end and VALUE_RANGE.end
const DATA_SIZE: usize = VALUE_RANGE.end;
const BLOCK_SIZE: usize = METADATA_SIZE + DATA_SIZE;
type BlockBytes = [u8; BLOCK_SIZE];
type MetadataBytes = [u8; METADATA_SIZE];
type DataBytes = [u8; DATA_SIZE];
const DATA_RANGE: Range<usize> = METADATA_SIZE..METADATA_SIZE + DATA_SIZE;
// const INTERNAL_PADDING_RANGE: Range<usize> = RIGHT_RANGE.end..DATA_SIZE;
// const INTERNAL_PADDING_SIZE: usize = INTERNAL_PADDING_RANGE.end - INTERNAL_PADDING_RANGE.start;
// const LEAF_PADDING_RANGE: Range<usize> = VALUE_RANGE.end..DATA_SIZE;
// const LEAF_PADDING_SIZE: usize = LEAF_PADDING_RANGE.end - LEAF_PADDING_RANGE.start;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum NodeType {
    Internal = 0,
    Leaf = 1,
}

impl NodeType {
    pub fn from_u8(value: u8) -> Result<Self, String> {
        // TODO: identify some useful structured serialization tooling we use
        // TODO: find a better way to tie serialization values to enumerators
        match value {
            // ha!  feel free to laugh at this
            x if (NodeType::Internal as u8 == x) => Ok(NodeType::Internal),
            x if (NodeType::Leaf as u8 == x) => Ok(NodeType::Leaf),
            other => panic!("unknown NodeType value: {other}"),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            NodeType::Internal => NodeType::Internal as u8,
            NodeType::Leaf => NodeType::Leaf as u8,
        }
    }
}

// impl NodeType {
//     const TYPE_TO_VALUE: HashMap<NodeType, u8> = HashMap::from([
//         (NodeType::Internal, 0),
//         (NodeType::Leaf, 1),
//     ]);
//
//     fn value(&self) -> u8 {
//         let map = Self::TYPE_TO_VALUE;
//         // TODO: this seems pretty clearly the wrong way, probably
//         let value = map.get(self);
//         if value.is_some() {
//             return 3;
//         }
//         panic!("no value for NodeType: {self:?}");
//     }
// }

#[allow(clippy::needless_pass_by_value)]
fn sha256_num<T: num_traits::ops::bytes::ToBytes>(input: T) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input.to_be_bytes());

    hasher.finalize()
}

fn sha256_bytes(input: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input);

    hasher.finalize()
}

fn internal_hash(left_hash: &Hash, right_hash: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(b"\x02");
    hasher.update(left_hash);
    hasher.update(right_hash);

    hasher.finalize()
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum InsertLocation {
    Auto,
    AsRoot,
    Leaf { index: TreeIndex, side: Side },
}

const NULL_PARENT: TreeIndex = 0xffff_ffffu32;

#[derive(Debug, PartialEq)]
pub struct NodeMetadata {
    pub node_type: NodeType,
    pub dirty: bool,
}

impl NodeMetadata {
    pub fn from_bytes(blob: MetadataBytes) -> Result<Self, String> {
        // TODO: could save 1-2% of tree space by packing (and maybe don't do that)
        // TODO: identify some useful structured serialization tooling we use
        Ok(Self {
            node_type: Self::node_type_from_bytes(blob)?,
            dirty: Self::dirty_from_bytes(blob)?,
        })
    }

    pub fn to_bytes(&self) -> MetadataBytes {
        [self.node_type.to_u8(), u8::from(self.dirty)]
    }

    pub fn node_type_from_bytes(blob: MetadataBytes) -> Result<NodeType, String> {
        NodeType::from_u8(blob[0])
    }

    pub fn dirty_from_bytes(blob: MetadataBytes) -> Result<bool, String> {
        match blob[1] {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!("invalid dirty value: {other}")),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Node {
    parent: Parent,
    hash: Hash,
    specific: NodeSpecific,
}

#[derive(Debug, PartialEq)]
pub enum NodeSpecific {
    Internal { left: TreeIndex, right: TreeIndex },
    Leaf { key: KvId, value: KvId },
}

impl NodeSpecific {
    pub fn sibling_index(&self, index: TreeIndex) -> TreeIndex {
        let NodeSpecific::Internal { right, left } = self else {
            panic!("unable to get sibling index from a leaf")
        };

        match index {
            x if (x == *right) => *left,
            x if (x == *left) => *right,
            _ => panic!("index not a child: {index}"),
        }
    }
}

impl Node {
    // fn discriminant(&self) -> u8 {
    //     unsafe { *(self as *const Self as *const u8) }
    // }

    pub fn from_bytes(metadata: &NodeMetadata, blob: DataBytes) -> Result<Self, String> {
        Ok(Self {
            parent: Self::parent_from_bytes(&blob),
            hash: blob[HASH_RANGE].try_into().unwrap(),
            specific: match metadata.node_type {
                NodeType::Internal => NodeSpecific::Internal {
                    left: TreeIndex::from_be_bytes(blob[LEFT_RANGE].try_into().unwrap()),
                    right: TreeIndex::from_be_bytes(blob[RIGHT_RANGE].try_into().unwrap()),
                },
                NodeType::Leaf => NodeSpecific::Leaf {
                    key: KvId::from_be_bytes(blob[KEY_RANGE].try_into().unwrap()),
                    value: KvId::from_be_bytes(blob[VALUE_RANGE].try_into().unwrap()),
                },
            },
        })
    }

    fn parent_from_bytes(blob: &DataBytes) -> Parent {
        let parent_integer = TreeIndex::from_be_bytes(blob[PARENT_RANGE].try_into().unwrap());
        match parent_integer {
            NULL_PARENT => None,
            _ => Some(parent_integer),
        }
    }
    pub fn to_bytes(&self) -> DataBytes {
        let mut blob: DataBytes = [0; DATA_SIZE];
        match self {
            Node {
                parent,
                specific: NodeSpecific::Internal { left, right },
                hash,
            } => {
                let parent_integer = match parent {
                    None => NULL_PARENT,
                    Some(parent) => *parent,
                };
                blob[HASH_RANGE].copy_from_slice(hash);
                blob[PARENT_RANGE].copy_from_slice(&parent_integer.to_be_bytes());
                blob[LEFT_RANGE].copy_from_slice(&left.to_be_bytes());
                blob[RIGHT_RANGE].copy_from_slice(&right.to_be_bytes());
            }
            Node {
                parent,
                specific: NodeSpecific::Leaf { key, value },
                hash,
            } => {
                let parent_integer = match parent {
                    None => NULL_PARENT,
                    Some(parent) => *parent,
                };
                blob[HASH_RANGE].copy_from_slice(hash);
                blob[PARENT_RANGE].copy_from_slice(&parent_integer.to_be_bytes());
                blob[KEY_RANGE].copy_from_slice(&key.to_be_bytes());
                blob[VALUE_RANGE].copy_from_slice(&value.to_be_bytes());
            }
        }

        blob
    }

    pub fn to_dot(&self, index: TreeIndex) -> DotLines {
        // TODO: can this be done without introducing a blank line?
        let node_to_parent = match self.parent {
            Some(parent) => format!("node_{index} -> node_{parent};"),
            None => String::new(),
        };

        match self.specific {
            NodeSpecific::Internal {left, right} => DotLines{
                nodes: vec![
                    format!("node_{index} [label=\"{index}\"]"),
                ],
                connections: vec![
                    format!("node_{index} -> node_{left};"),
                    format!("node_{index} -> node_{right};"),
                    node_to_parent,
                ],
                pair_boxes: vec![
                    format!("node [shape = box]; {{rank = same; node_{left}->node_{right}[style=invis]; rankdir = LR}}"),
                ],
                note: String::new(),
            },
            NodeSpecific::Leaf {key, value} => DotLines{
                nodes: vec![
                    format!("node_{index} [shape=box, label=\"{index}\\nvalue: {key}\\nvalue: {value}\"];"),
                ],
                connections: vec![node_to_parent],
                pair_boxes: vec![],
                note: String::new(),
            },
        }
    }
}

fn block_range(index: TreeIndex) -> Range<usize> {
    let block_start = index as usize * BLOCK_SIZE;
    block_start..block_start + BLOCK_SIZE
}

// TODO: does not enforce matching metadata node type and node enumeration type
pub struct Block {
    metadata: NodeMetadata,
    node: Node,
}

impl Block {
    pub fn to_bytes(&self) -> BlockBytes {
        let mut blob: BlockBytes = [0; BLOCK_SIZE];
        blob[METADATA_RANGE].copy_from_slice(&self.metadata.to_bytes());
        blob[DATA_RANGE].copy_from_slice(&self.node.to_bytes());

        blob
    }

    pub fn from_bytes(blob: BlockBytes) -> Result<Block, String> {
        let metadata_blob: MetadataBytes = blob[METADATA_RANGE].try_into().unwrap();
        let data_blob: DataBytes = blob[DATA_RANGE].try_into().unwrap();
        let metadata = NodeMetadata::from_bytes(metadata_blob)
            .map_err(|message| format!("failed loading metadata: {message})"))?;
        let node = Node::from_bytes(&metadata, data_blob)
            .map_err(|message| format!("failed loading node: {message})"))?;

        Ok(Block { metadata, node })
    }
}

fn get_free_indexes(blob: &[u8]) -> Vec<TreeIndex> {
    let index_count = blob.len() / BLOCK_SIZE;

    if index_count == 0 {
        return vec![];
    }

    let mut seen_indexes: Vec<bool> = vec![false; index_count];

    for (index, _) in MerkleBlobLeftChildFirstIterator::new(blob) {
        seen_indexes[index as usize] = true;
    }

    let mut free_indexes: Vec<TreeIndex> = vec![];
    for (index, seen) in seen_indexes.iter().enumerate() {
        if !seen {
            free_indexes.push(index as TreeIndex);
        }
    }

    free_indexes
}

fn get_keys_values_indexes(blob: &[u8]) -> HashMap<KvId, TreeIndex> {
    let index_count = blob.len() / BLOCK_SIZE;

    let mut key_to_index: HashMap<KvId, TreeIndex> = HashMap::default();

    if index_count == 0 {
        return key_to_index;
    }

    for (index, block) in MerkleBlobLeftChildFirstIterator::new(blob) {
        if let NodeSpecific::Leaf { key, .. } = block.node.specific {
            key_to_index.insert(key, index);
        }
    }

    key_to_index
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "MerkleBlob"))]
#[derive(Debug)]
pub struct MerkleBlob {
    blob: Vec<u8>,
    free_indexes: Vec<TreeIndex>,
    key_to_index: HashMap<KvId, TreeIndex>,
    next_index_to_allocate: TreeIndex,
}

impl MerkleBlob {
    pub fn new(blob: Vec<u8>) -> Result<Self, String> {
        let length = blob.len();
        let block_count = length / BLOCK_SIZE;
        let remainder = length % BLOCK_SIZE;
        if remainder != 0 {
            return Err(format!(
                "blob length must be a multiple of block count, found extra bytes: {remainder}"
            ));
        }

        // TODO: stop double tree traversals here
        let free_indexes = get_free_indexes(&blob);
        let key_to_index = get_keys_values_indexes(&blob);

        Ok(Self {
            blob,
            free_indexes,
            key_to_index,
            next_index_to_allocate: block_count as TreeIndex,
        })
    }

    pub fn insert(
        &mut self,
        key: KvId,
        value: KvId,
        hash: &Hash,
        insert_location: InsertLocation,
    ) -> Result<(), String> {
        let insert_location = match insert_location {
            InsertLocation::Auto => self.get_random_insert_location_by_kvid(key)?,
            _ => insert_location,
        };

        match insert_location {
            InsertLocation::Auto => {
                panic!("this should have been caught and processed above")
            }
            InsertLocation::AsRoot => {
                if !self.key_to_index.is_empty() {
                    return Err("requested insertion at root but tree not empty".to_string());
                };
                self.insert_first(key, value, hash);
            }
            InsertLocation::Leaf { index, side } => {
                let old_leaf = self.get_node(index)?;
                let NodeSpecific::Leaf { .. } = old_leaf.specific else {
                    panic!("requested insertion at leaf but found internal node")
                };

                let internal_node_hash = match side {
                    Side::Left => internal_hash(hash, &old_leaf.hash),
                    Side::Right => internal_hash(&old_leaf.hash, hash),
                };

                if self.key_to_index.len() == 1 {
                    self.insert_second(key, value, hash, &old_leaf, &internal_node_hash, &side)?;
                } else {
                    self.insert_third_or_later(
                        key,
                        value,
                        hash,
                        &old_leaf,
                        index,
                        &internal_node_hash,
                        &side,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn insert_first(&mut self, key: KvId, value: KvId, hash: &Hash) {
        let new_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node {
                parent: None,
                specific: NodeSpecific::Leaf { key, value },
                hash: *hash,
            },
        };

        self.blob.extend(new_leaf_block.to_bytes());

        self.key_to_index.insert(key, 0);
        self.free_indexes.clear();
        self.next_index_to_allocate = 1;
    }

    fn insert_second(
        &mut self,
        key: KvId,
        value: KvId,
        hash: &Hash,
        old_leaf: &Node,
        internal_node_hash: &Hash,
        side: &Side,
    ) -> Result<(), String> {
        self.blob.clear();
        self.blob.resize(BLOCK_SIZE * 3, 0);
        self.free_indexes.clear();

        let new_internal_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node {
                parent: None,
                specific: NodeSpecific::Internal { left: 1, right: 2 },
                hash: *internal_node_hash,
            },
        };

        self.insert_entry_to_blob(0, new_internal_block.to_bytes())?;

        let NodeSpecific::Leaf {
            key: old_leaf_key,
            value: old_leaf_value,
        } = old_leaf.specific
        else {
            return Err("old leaf unexpectedly not a leaf".to_string());
        };
        let nodes = [
            (
                match side {
                    Side::Left => 2,
                    Side::Right => 1,
                },
                Node {
                    parent: Some(0),
                    specific: NodeSpecific::Leaf {
                        key: old_leaf_key,
                        value: old_leaf_value,
                    },
                    hash: old_leaf.hash,
                },
            ),
            (
                match side {
                    Side::Left => 1,
                    Side::Right => 2,
                },
                Node {
                    parent: Some(0),
                    specific: NodeSpecific::Leaf { key, value },
                    hash: *hash,
                },
            ),
        ];

        for (index, node) in nodes {
            let block = Block {
                metadata: NodeMetadata {
                    node_type: NodeType::Leaf,
                    dirty: false,
                },
                node,
            };

            self.insert_entry_to_blob(index, block.to_bytes())?;
            let NodeSpecific::Leaf { key: this_key, .. } = block.node.specific else {
                return Err("new block unexpectedly not a leaf".to_string());
            };
            self.key_to_index.insert(this_key, index);
        }

        self.next_index_to_allocate = 3;

        Ok(())
    }

    // TODO: no really, actually consider the too many arguments complaint
    #[allow(clippy::too_many_arguments)]
    fn insert_third_or_later(
        &mut self,
        key: KvId,
        value: KvId,
        hash: &Hash,
        old_leaf: &Node,
        old_leaf_index: TreeIndex,
        internal_node_hash: &Hash,
        side: &Side,
    ) -> Result<(), String> {
        let new_leaf_index = self.get_new_index();
        let new_internal_node_index = self.get_new_index();

        let new_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node {
                parent: Some(new_internal_node_index),
                specific: NodeSpecific::Leaf { key, value },
                hash: *hash,
            },
        };
        self.insert_entry_to_blob(new_leaf_index, new_leaf_block.to_bytes())?;

        let (left_index, right_index) = match side {
            Side::Left => (new_leaf_index, old_leaf_index),
            Side::Right => (old_leaf_index, new_leaf_index),
        };
        let new_internal_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node {
                parent: old_leaf.parent,
                specific: NodeSpecific::Internal {
                    left: left_index,
                    right: right_index,
                },
                hash: *internal_node_hash,
            },
        };
        self.insert_entry_to_blob(new_internal_node_index, new_internal_block.to_bytes())?;

        let Some(old_parent_index) = old_leaf.parent else {
            panic!("root found when not expected: {key:?} {value:?} {hash:?}")
        };

        let mut block = Block::from_bytes(self.get_block_bytes(old_leaf_index)?)?;
        block.node.parent = Some(new_internal_node_index);
        self.insert_entry_to_blob(old_leaf_index, block.to_bytes())?;

        let mut old_parent_block = Block::from_bytes(self.get_block_bytes(old_parent_index)?)?;
        if let NodeSpecific::Internal {
            ref mut left,
            ref mut right,
            ..
        } = old_parent_block.node.specific
        {
            if old_leaf_index == *left {
                *left = new_internal_node_index;
            } else if old_leaf_index == *right {
                *right = new_internal_node_index;
            } else {
                panic!("child not a child of its parent");
            }
        } else {
            panic!("expected internal node but found leaf");
        };

        self.insert_entry_to_blob(old_parent_index, old_parent_block.to_bytes())?;

        self.mark_lineage_as_dirty(old_parent_index)?;
        self.key_to_index.insert(key, new_leaf_index);

        Ok(())
    }

    pub fn delete(&mut self, key: KvId) -> Result<(), String> {
        let leaf_index = *self
            .key_to_index
            .get(&key)
            .ok_or(format!("unknown key: {key}"))?;
        let leaf = self.get_node(leaf_index)?;

        // TODO: maybe some common way to indicate/perform sanity double checks?
        let NodeSpecific::Leaf { .. } = leaf.specific else {
            panic!("key to index cache resulted in internal node")
        };
        self.key_to_index.remove(&key);

        let Some(parent_index) = leaf.parent else {
            self.free_indexes.clear();
            self.next_index_to_allocate = 0;
            self.blob.clear();
            return Ok(());
        };

        self.free_indexes.push(leaf_index);
        let parent = self.get_node(parent_index)?;
        // TODO: kinda implicit that we 'check' that parent is internal inside .sibling_index()
        let sibling_index = parent.specific.sibling_index(leaf_index);
        let mut sibling_block = self.get_block(sibling_index)?;

        let Some(grandparent_index) = parent.parent else {
            sibling_block.node.parent = None;
            self.insert_entry_to_blob(0, sibling_block.to_bytes())?;

            match sibling_block.node.specific {
                NodeSpecific::Leaf { key, .. } => {
                    self.key_to_index.insert(key, 0);
                }
                NodeSpecific::Internal { left, right } => {
                    for child_index in [left, right] {
                        let mut block = self.get_block(child_index)?;
                        block.node.parent = Some(0);
                        self.insert_entry_to_blob(child_index, block.to_bytes())?;
                    }
                }
            };

            self.free_indexes.push(sibling_index);

            return Ok(());
        };

        self.free_indexes.push(parent_index);
        let mut grandparent_block = self.get_block(grandparent_index)?;

        sibling_block.node.parent = Some(grandparent_index);
        self.insert_entry_to_blob(sibling_index, sibling_block.to_bytes())?;

        if let NodeSpecific::Internal {
            ref mut left,
            ref mut right,
            ..
        } = grandparent_block.node.specific
        {
            match parent_index {
                x if x == *left => *left = sibling_index,
                x if x == *right => *right = sibling_index,
                _ => panic!("parent not a child a grandparent"),
            }
        } else {
            panic!("grandparent not an internal node")
        }
        self.insert_entry_to_blob(grandparent_index, grandparent_block.to_bytes())?;

        self.mark_lineage_as_dirty(grandparent_index)?;

        Ok(())
    }

    pub fn upsert(&mut self, key: KvId, value: KvId, new_hash: &Hash) -> Result<(), String> {
        let Some(leaf_index) = self.key_to_index.get(&key) else {
            self.insert(key, value, new_hash, InsertLocation::Auto)?;
            return Ok(());
        };

        let mut block = self.get_block(*leaf_index)?;
        if let NodeSpecific::Leaf {
            value: ref mut inplace_value,
            ..
        } = block.node.specific
        {
            block.node.hash.clone_from(new_hash);
            *inplace_value = value;
        } else {
            panic!("expected internal node but found leaf");
        }
        self.insert_entry_to_blob(*leaf_index, block.to_bytes())?;

        if let Some(parent) = block.node.parent {
            self.mark_lineage_as_dirty(parent)?;
        }

        Ok(())
    }

    pub fn check(&self) -> Result<(), String> {
        let mut leaf_count: usize = 0;
        let mut internal_count: usize = 0;

        for (index, block) in self {
            match block.node.specific {
                NodeSpecific::Internal { .. } => internal_count += 1,
                NodeSpecific::Leaf { key, .. } => {
                    leaf_count += 1;
                    let cached_index = self
                        .key_to_index
                        .get(&key)
                        .ok_or(format!("key not in key to index cache: {key:?}"))?;
                    assert_eq!(
                        *cached_index, index,
                        "key to index cache for {key:?} should be {index:?} got: {cached_index:?}"
                    );
                    // TODO: consider what type free indexes should be
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
            total_count, extend_index as usize,
            "expected total node count {extend_index:?} found: {total_count:?}",
        );

        Ok(())
        // TODO: check parent/child bidirectional accuracy
    }

    // fn update_parent(&mut self, index: TreeIndex, parent: Option<TreeIndex>) -> Result<(), String> {
    //     let range = self.get_block_range(index);
    //
    //     let mut node = self.get_node(index)?;
    //     node.parent = parent;
    //     self.blob[range].copy_from_slice(&node.to_bytes());
    //
    //     Ok(())
    // }

    // fn update_left(&mut self, index: TreeIndex, left: Option<TreeIndex>) -> Result<(), String> {
    //     let range = self.get_block_range(index);
    //
    //     let mut node = self.get_node(index)?;
    //     node.left = left;
    //     self.blob[range].copy_from_slice(&node.to_bytes());
    //
    //     Ok(())
    // }

    fn mark_lineage_as_dirty(&mut self, index: TreeIndex) -> Result<(), String> {
        let mut next_index = Some(index);

        while let Some(this_index) = next_index {
            let mut block = Block::from_bytes(self.get_block_bytes(this_index)?)?;

            if block.metadata.dirty {
                return Ok(());
            }

            block.metadata.dirty = true;
            self.insert_entry_to_blob(this_index, block.to_bytes())?;
            next_index = block.node.parent;
        }

        Ok(())
    }

    fn get_new_index(&mut self) -> TreeIndex {
        match self.free_indexes.pop() {
            None => {
                // TODO: should this extend...?
                // TODO: should this update free indexes...?
                self.next_index_to_allocate += 1;
                self.next_index_to_allocate - 1
            }
            Some(new_index) => new_index,
        }
    }

    fn get_random_insert_location_by_seed(
        &self,
        seed_bytes: &[u8],
    ) -> Result<InsertLocation, String> {
        let mut seed_bytes = Vec::from(seed_bytes);

        if self.blob.is_empty() {
            return Ok(InsertLocation::AsRoot);
        }

        let side = if (seed_bytes
            .last()
            .ok_or("zero-length seed bytes not allowed")?
            & 1 << 7)
            == 0
        {
            Side::Left
        } else {
            Side::Right
        };
        let mut next_index: TreeIndex = 0;
        let mut node = self.get_node(next_index)?;

        loop {
            for byte in &seed_bytes {
                for bit in 0..8 {
                    match node.specific {
                        NodeSpecific::Leaf { .. } => {
                            return Ok(InsertLocation::Leaf {
                                index: next_index,
                                side,
                            })
                        }
                        NodeSpecific::Internal { left, right, .. } => {
                            next_index = if byte & (1 << bit) != 0 { left } else { right };
                            node = self.get_node(next_index)?;
                        }
                    }
                }
            }

            seed_bytes = sha256_bytes(&seed_bytes).into();
        }
    }

    fn get_random_insert_location_by_kvid(&self, seed: KvId) -> Result<InsertLocation, String> {
        let seed = sha256_num(seed);

        self.get_random_insert_location_by_seed(&seed)
    }

    fn extend_index(&self) -> TreeIndex {
        let blob_length = self.blob.len();
        let remainder = blob_length % BLOCK_SIZE;
        assert_eq!(remainder, 0, "blob length {blob_length:?} not a multiple of {BLOCK_SIZE:?}, remainder: {remainder:?}");

        (self.blob.len() / BLOCK_SIZE) as TreeIndex
    }

    fn insert_entry_to_blob(
        &mut self,
        index: TreeIndex,
        block_bytes: BlockBytes,
    ) -> Result<(), String> {
        let extend_index = self.extend_index();
        match index.cmp(&extend_index) {
            Ordering::Greater => return Err(format!("block index out of range: {index}")),
            Ordering::Equal => self.blob.extend_from_slice(&block_bytes),
            Ordering::Less => {
                self.blob[block_range(index)].copy_from_slice(&block_bytes);
            }
        }

        Ok(())
    }

    fn get_block(&self, index: TreeIndex) -> Result<Block, String> {
        Block::from_bytes(self.get_block_bytes(index)?)
    }

    // fn get_block_slice(&self, index: TreeIndex) -> Result<&mut BlockBytes, String> {
    //     let metadata_start = index as usize * BLOCK_SIZE;
    //     let data_start = metadata_start + METADATA_SIZE;
    //     let end = data_start + DATA_SIZE;
    //
    //     self.blob
    //         .get(metadata_start..end)
    //         .ok_or(format!("index out of bounds: {index}"))?
    //         .try_into()
    //         .map_err(|e| format!("failed getting block {index}: {e}"))
    // }

    fn get_block_bytes(&self, index: TreeIndex) -> Result<BlockBytes, String> {
        self.blob
            .get(block_range(index))
            .ok_or(format!("block index out of bounds: {index}"))?
            .try_into()
            .map_err(|e| format!("failed getting block {index}: {e}"))
    }

    pub fn get_node(&self, index: TreeIndex) -> Result<Node, String> {
        // TODO: use Block::from_bytes()
        // TODO: handle invalid indexes?
        // TODO: handle overflows?
        let block = self.get_block_bytes(index)?;
        let metadata_blob: MetadataBytes = block[METADATA_RANGE].try_into().unwrap();
        let data_blob: DataBytes = block[DATA_RANGE].try_into().unwrap();
        let metadata = NodeMetadata::from_bytes(metadata_blob)
            .map_err(|message| format!("failed loading metadata: {message})"))?;

        Node::from_bytes(&metadata, data_blob)
            .map_err(|message| format!("failed loading node: {message}"))
    }

    pub fn get_parent_index(&self, index: TreeIndex) -> Result<Parent, String> {
        let block = self.get_block_bytes(index)?;

        Ok(Node::parent_from_bytes(
            block[DATA_RANGE].try_into().unwrap(),
        ))
    }

    pub fn get_lineage(&self, index: TreeIndex) -> Result<Vec<Node>, String> {
        // TODO: what about an index that happens to be the null index?  a question for everywhere i guess
        let mut next_index = Some(index);
        let mut lineage = vec![];

        while let Some(this_index) = next_index {
            let node = self.get_node(this_index)?;
            next_index = node.parent;
            lineage.push(node);
        }

        Ok(lineage)
    }

    pub fn get_lineage_indexes(&self, index: TreeIndex) -> Result<Vec<TreeIndex>, String> {
        // TODO: yep, this 'optimization' might be overkill, and should be speed compared regardless
        // TODO: what about an index that happens to be the null index?  a question for everywhere i guess
        let mut next_index = Some(index);
        let mut lineage: Vec<TreeIndex> = vec![];

        while let Some(this_index) = next_index {
            lineage.push(this_index);
            next_index = self.get_parent_index(this_index)?;
        }

        Ok(lineage)
    }

    pub fn to_dot(&self) -> DotLines {
        let mut result = DotLines::new();
        for (index, block) in self {
            result.push(block.node.to_dot(index));
        }

        result
    }

    pub fn iter(&self) -> MerkleBlobLeftChildFirstIterator<'_> {
        <&Self as IntoIterator>::into_iter(self)
    }

    pub fn calculate_lazy_hashes(&mut self) -> Result<(), String> {
        // TODO: really want a truncated traversal, not filter
        // TODO: yeah, storing the whole set of blocks via collect is not great
        for (index, mut block) in self
            .iter()
            .filter(|(_, block)| block.metadata.dirty)
            .collect::<Vec<_>>()
        {
            let NodeSpecific::Internal { left, right } = block.node.specific else {
                panic!("leaves should not be dirty")
            };
            // TODO: obviously inefficient to re-get/deserialize these blocks inside
            //       an iteration that's already doing that
            let left = self.get_block(left)?;
            let right = self.get_block(right)?;
            // TODO: wrap this up in Block maybe? just to have 'control' of dirty being 'accurate'
            block.node.hash = internal_hash(&left.node.hash, &right.node.hash);
            block.metadata.dirty = false;
            self.insert_entry_to_blob(index, block.to_bytes())?;
        }

        Ok(())
    }

    #[allow(unused)]
    fn relocate_node(&mut self, source: TreeIndex, destination: TreeIndex) -> Result<(), String> {
        let extend_index = self.extend_index();
        // TODO: perhaps relocation of root should be allowed for some use
        if source == 0 {
            return Err("relocation of the root and index zero is not allowed".to_string());
        };
        assert!(source < extend_index);
        assert!(!self.free_indexes.contains(&source));
        assert!(destination <= extend_index);
        assert!(destination == extend_index || self.free_indexes.contains(&destination));

        let source_block = self.get_block(source).unwrap();
        if let Some(parent) = source_block.node.parent {
            let mut parent_block = self.get_block(parent).unwrap();
            let NodeSpecific::Internal {
                ref mut left,
                ref mut right,
            } = parent_block.node.specific
            else {
                panic!();
            };
            match source {
                x if x == *left => *left = destination,
                x if x == *right => *right = destination,
                _ => panic!(),
            }
            self.insert_entry_to_blob(parent, parent_block.to_bytes())
                .unwrap();
        }

        match source_block.node.specific {
            NodeSpecific::Leaf { key, .. } => {
                self.key_to_index.insert(key, destination);
            }
            NodeSpecific::Internal { left, right, .. } => {
                for child in [left, right] {
                    let mut block = self.get_block(child).unwrap();
                    block.node.parent = Some(destination);
                    self.insert_entry_to_blob(child, block.to_bytes()).unwrap();
                }
            }
        }

        self.free_indexes.push(source);

        Ok(())
    }

    #[allow(unused)]
    fn rebuild(&mut self) -> Result<(), String> {
        panic!();
        // TODO: could make insert_entry_to_blob a free function and not need to make
        //       a merkle blob here?  maybe?
        let mut new = Self::new(Vec::new())?;
        for (index, block) in MerkleBlobParentFirstIterator::new(&self.blob).enumerate() {
            // new.insert_entry_to_blob(index, )?
        }
        self.blob = new.blob;

        Ok(())
    }

    #[allow(unused)]
    fn get_key_value_map(&self) -> HashMap<KvId, KvId> {
        let mut key_value = HashMap::new();
        for (key, index) in &self.key_to_index {
            let NodeSpecific::Leaf { value, .. } = self.get_node(*index).unwrap().specific else {
                panic!()
            };
            key_value.insert(*key, value);
        }

        key_value
    }
}

impl PartialEq for MerkleBlob {
    fn eq(&self, other: &Self) -> bool {
        // TODO: should we check the indexes?
        for ((_, self_block), (_, other_block)) in zip(self, other) {
            if (self_block.metadata.dirty || other_block.metadata.dirty)
                || self_block.node.hash != other_block.node.hash
                // TODO: isn't only a leaf supposed to check this?
                || self_block.node.specific != other_block.node.specific
            {
                return false;
            }
        }

        true
    }
}

impl<'a> IntoIterator for &'a MerkleBlob {
    // TODO: review efficiency in whatever use cases we end up with, vs Item = Node etc
    type Item = (TreeIndex, Block);
    type IntoIter = MerkleBlobLeftChildFirstIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        // TODO: review types around this to avoid copying
        MerkleBlobLeftChildFirstIterator::new(&self.blob[..])
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

        Ok(Self::new(Vec::from(slice)).unwrap())
    }

    #[pyo3(name = "insert")]
    pub fn py_insert(&mut self, key: KvId, value: KvId, hash: Hash) -> PyResult<()> {
        // TODO: consider the error
        // TODO: expose insert location
        self.insert(key, value, &hash, InsertLocation::Auto)
            .unwrap();

        Ok(())
    }

    #[pyo3(name = "delete")]
    pub fn py_delete(&mut self, key: KvId) -> PyResult<()> {
        // TODO: consider the error
        self.delete(key).unwrap();

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
                index: 0,
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

            match block.node.specific {
                NodeSpecific::Leaf { .. } => return Some((item.index, block)),
                NodeSpecific::Internal { left, right } => {
                    if item.visited {
                        return Some((item.index, block));
                    };

                    self.deque.push_front(MerkleBlobLeftChildFirstIteratorItem {
                        visited: true,
                        index: item.index,
                    });
                    self.deque.push_front(MerkleBlobLeftChildFirstIteratorItem {
                        visited: false,
                        index: right,
                    });
                    self.deque.push_front(MerkleBlobLeftChildFirstIteratorItem {
                        visited: false,
                        index: left,
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
            deque.push_back(0);
        }

        Self { blob, deque }
    }
}

impl Iterator for MerkleBlobParentFirstIterator<'_> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parents before children

        loop {
            let index = self.deque.pop_front()?;
            let block_bytes: BlockBytes = self.blob[block_range(index)].try_into().unwrap();
            let block = Block::from_bytes(block_bytes).unwrap();

            match block.node.specific {
                NodeSpecific::Leaf { .. } => return Some(block),
                NodeSpecific::Internal { left, right } => {
                    self.deque.push_front(right);
                    self.deque.push_front(left);
                }
            }
        }
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
            deque.push_back(0);
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

            match block.node.specific {
                NodeSpecific::Leaf { .. } => return Some(block),
                NodeSpecific::Internal { left, right } => {
                    self.deque.push_back(left);
                    self.deque.push_back(right);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use hex_literal::hex;
    use rstest::{fixture, rstest};
    use std::time::{Duration, Instant};

    // const EXAMPLE_BLOB: [u8; 138] = hex!("0001ffffffff00000001000000020c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000000405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000001415161718191a1b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b");
    // const HASH: Hash = [
    //     12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
    //     35, 36, 37, 38, 39, 40, 41, 42, 43,
    // ];
    //
    // const EXAMPLE_ROOT: Node = Node {
    //     parent: None,
    //     specific: NodeSpecific::Internal { left: 1, right: 2 },
    //     hash: HASH,
    //     index: 0,
    // };
    // const EXAMPLE_ROOT_METADATA: NodeMetadata = NodeMetadata {
    //     node_type: NodeType::Internal,
    //     dirty: true,
    // };
    // const EXAMPLE_LEFT_LEAF: Node = Node {
    //     parent: Some(0),
    //     specific: NodeSpecific::Leaf {
    //         key: 0x0405_0607_0809_0A0B,
    //         value: 0x1415_1617_1819_1A1B,
    //     },
    //     hash: HASH,
    //     index: 1,
    // };
    // const EXAMPLE_LEFT_LEAF_METADATA: NodeMetadata = NodeMetadata {
    //     node_type: NodeType::Leaf,
    //     dirty: false,
    // };
    // const EXAMPLE_RIGHT_LEAF: Node = Node {
    //     parent: Some(0),
    //     specific: NodeSpecific::Leaf {
    //         key: 0x2425_2627_2829_2A2B,
    //         value: 0x3435_3637_3839_3A3B,
    //     },
    //     hash: HASH,
    //     index: 2,
    // };
    // const EXAMPLE_RIGHT_LEAF_METADATA: NodeMetadata = NodeMetadata {
    //     node_type: NodeType::Leaf,
    //     dirty: false,
    // };

    // fn example_merkle_blob() -> MerkleBlob {
    //     MerkleBlob::new(Vec::from(EXAMPLE_BLOB)).unwrap()
    // }

    #[allow(unused)]
    fn normalized_blob(merkle_blob: &MerkleBlob) -> Vec<u8> {
        let mut new = MerkleBlob::new(merkle_blob.blob.clone()).unwrap();

        new.calculate_lazy_hashes();
        new.rebuild();

        new.blob
    }

    #[test]
    fn test_node_type_serialized_values() {
        // TODO: can i make sure we cover all variants?
        assert_eq!(NodeType::Internal as u8, 0);
        assert_eq!(NodeType::Leaf as u8, 1);

        for node_type in [NodeType::Internal, NodeType::Leaf] {
            assert_eq!(node_type.to_u8(), node_type.clone() as u8,);
            assert_eq!(
                NodeType::from_u8(node_type.clone() as u8).unwrap(),
                node_type,
            );
        }
    }

    #[test]
    fn test_internal_hash() {
        // TODO: yeah, various questions around this and how to express 'this is dl internal hash'
        //       without silly repetition.  maybe just a use as.
        // in Python: Program.to((left_hash, right_hash)).get_tree_hash_precalc(left_hash, right_hash)
        let left: Hash = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ];
        let right: Hash = [
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53,
            54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
        ];
        assert_eq!(
            internal_hash(&left, &right),
            clvm_utils::tree_hash_pair(
                clvm_utils::TreeHash::new(left),
                clvm_utils::TreeHash::new(right)
            )
            .to_bytes(),
        );
    }

    #[rstest]
    fn test_node_metadata_from_to(
        #[values(false, true)] dirty: bool,
        // TODO: can we make sure we cover all variants
        #[values(NodeType::Internal, NodeType::Leaf)] node_type: NodeType,
    ) {
        let bytes: [u8; 2] = [node_type.to_u8(), dirty as u8];
        let object = NodeMetadata::from_bytes(bytes).unwrap();
        assert_eq!(object, NodeMetadata { node_type, dirty },);
        assert_eq!(object.to_bytes(), bytes);
        assert_eq!(
            NodeMetadata::node_type_from_bytes(bytes).unwrap(),
            object.node_type
        );
        assert_eq!(NodeMetadata::dirty_from_bytes(bytes).unwrap(), object.dirty);
    }

    // #[test]
    // fn test_load_a_python_dump() {
    //     let merkle_blob = example_merkle_blob();
    //     merkle_blob.get_node(0).unwrap();
    //
    //     merkle_blob.check().unwrap();
    // }

    #[fixture]
    fn small_blob() -> MerkleBlob {
        let mut blob = MerkleBlob::new(vec![]).unwrap();

        blob.insert(
            0x0001_0203_0405_0607,
            0x1011_1213_1415_1617,
            &sha256_num(0x1020),
            InsertLocation::Auto,
        )
        .unwrap();

        blob.insert(
            0x2021_2223_2425_2627,
            0x3031_3233_3435_3637,
            &sha256_num(0x2030),
            InsertLocation::Auto,
        )
        .unwrap();

        blob
    }

    #[rstest]
    fn test_get_lineage(small_blob: MerkleBlob) {
        let lineage = small_blob.get_lineage(2).unwrap();
        for node in &lineage {
            println!("{node:?}");
        }
        assert_eq!(lineage.len(), 2);
        let last_node = lineage.last().unwrap();
        assert_eq!(last_node.parent, None);

        small_blob.check().unwrap();
    }

    #[rstest]
    #[case::right(0, 2, Side::Left)]
    #[case::left(0xff, 1, Side::Right)]
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

        small_blob.check().unwrap();
    }

    // #[test]
    // fn test_build_blob_and_read() {
    //     let mut blob: Vec<u8> = Vec::new();
    //
    //     blob.extend(EXAMPLE_ROOT_METADATA.to_bytes());
    //     blob.extend(EXAMPLE_ROOT.to_bytes());
    //     blob.extend(EXAMPLE_LEFT_LEAF_METADATA.to_bytes());
    //     blob.extend(EXAMPLE_LEFT_LEAF.to_bytes());
    //     blob.extend(EXAMPLE_RIGHT_LEAF_METADATA.to_bytes());
    //     blob.extend(EXAMPLE_RIGHT_LEAF.to_bytes());
    //
    //     assert_eq!(blob, Vec::from(EXAMPLE_BLOB));
    //
    //     let merkle_blob = MerkleBlob::new(Vec::from(EXAMPLE_BLOB)).unwrap();
    //
    //     assert_eq!(merkle_blob.get_node(0).unwrap(), EXAMPLE_ROOT);
    //     assert_eq!(merkle_blob.get_node(1).unwrap(), EXAMPLE_LEFT_LEAF);
    //     assert_eq!(merkle_blob.get_node(2).unwrap(), EXAMPLE_RIGHT_LEAF);
    //
    //     merkle_blob.check().unwrap();
    // }

    // #[test]
    // fn test_build_merkle() {
    //     let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();
    //
    //     let (key, value) = EXAMPLE_LEFT_LEAF.key_value();
    //     merkle_blob
    //         .insert(key, value, &EXAMPLE_LEFT_LEAF.hash)
    //         .unwrap();
    //     let (key, value) = EXAMPLE_RIGHT_LEAF.key_value();
    //     merkle_blob
    //         .insert(key, value, &EXAMPLE_RIGHT_LEAF.hash)
    //         .unwrap();
    //
    //     // TODO: just hacking here to compare with the ~wrong~ simplified reference
    //     let mut root = Block::from_bytes(merkle_blob.get_block_bytes(0).unwrap(), 0).unwrap();
    //     root.metadata.dirty = true;
    //     root.node.hash = HASH;
    //     assert_eq!(root.metadata.node_type, NodeType::Internal);
    //     merkle_blob
    //         .insert_entry_to_blob(0, root.to_bytes())
    //         .unwrap();
    //
    //     assert_eq!(merkle_blob.blob, Vec::from(EXAMPLE_BLOB));
    //
    //     merkle_blob.check().unwrap();
    // }

    #[test]
    fn test_just_insert_a_bunch() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let mut total_time = Duration::new(0, 0);

        for i in 0..100_000 {
            let start = Instant::now();
            merkle_blob
                // TODO: yeah this hash is garbage
                .insert(i, i, &sha256_num(i), InsertLocation::Auto)
                .unwrap();
            let end = Instant::now();
            total_time += end.duration_since(start);

            // match i + 1 {
            //     2 => assert_eq!(merkle_blob.blob.len(), 3 * BLOCK_SIZE),
            //     3 => assert_eq!(merkle_blob.blob.len(), 5 * BLOCK_SIZE),
            //     _ => (),
            // }

            // let file = fs::File::create(format!("/home/altendky/tmp/mbt/rs/{i:0>4}")).unwrap();
            // let mut file = io::LineWriter::new(file);
            // for block in merkle_blob.blob.chunks(BLOCK_SIZE) {
            //     let mut s = String::new();
            //     for byte in block {
            //         s.push_str(&format!("{:02x}", byte));
            //     }
            //     s.push_str("\n");
            //     file.write_all(s.as_bytes()).unwrap();
            // }

            // fs::write(format!("/home/altendky/tmp/mbt/rs/{i:0>4}"), &merkle_blob.blob).unwrap();
        }
        // println!("{:?}", merkle_blob.blob)

        println!("total time: {total_time:?}");
        // TODO: check, well...  something

        merkle_blob.calculate_lazy_hashes().unwrap();

        merkle_blob.check().unwrap();
    }

    #[test]
    fn test_delete_in_reverse_creates_matching_trees() {
        const COUNT: usize = 10;
        let mut dots = vec![];

        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();
        let mut reference_blobs = vec![];

        let key_value_ids: [KvId; COUNT] = core::array::from_fn(|i| i as KvId);

        for key_value_id in key_value_ids {
            let hash: Hash = sha256_num(key_value_id);

            println!("inserting: {key_value_id}");
            merkle_blob.calculate_lazy_hashes().unwrap();
            reference_blobs.push(MerkleBlob::new(merkle_blob.blob.clone()).unwrap());
            merkle_blob
                .insert(key_value_id, key_value_id, &hash, InsertLocation::Auto)
                .unwrap();
            dots.push(merkle_blob.to_dot().dump());
        }

        merkle_blob.check().unwrap();

        for key_value_id in key_value_ids.iter().rev() {
            println!("deleting: {key_value_id}");
            merkle_blob.delete(*key_value_id).unwrap();
            merkle_blob.calculate_lazy_hashes().unwrap();
            assert_eq!(merkle_blob, reference_blobs[*key_value_id as usize]);
            dots.push(merkle_blob.to_dot().dump());
        }

        merkle_blob.check().unwrap();
    }

    // TODO: better conditional execution than the commenting i'm doing now
    #[allow(dead_code)]
    fn open_dot(lines: &mut DotLines) {
        use open;
        use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
        use url::Url;

        let mut url = Url::parse("http://edotor.net").unwrap();
        // https://edotor.net/?engine=dot#graph%20%7B%7D%0A -> graph {}
        url.query_pairs_mut().append_pair("engine", "dot");
        url.set_fragment(Some(
            &utf8_percent_encode(&lines.dump(), NON_ALPHANUMERIC).to_string(),
        ));
        open::that(url.as_str()).unwrap();
    }

    #[test]
    fn test_insert_first() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_id: KvId = 1;
        // open_dot(&mut merkle_blob.to_dot().set_note("empty"));
        merkle_blob
            .insert(
                key_value_id,
                key_value_id,
                &sha256_num(key_value_id),
                InsertLocation::Auto,
            )
            .unwrap();
        // open_dot(&mut merkle_blob.to_dot().set_note("first after"));

        merkle_blob.check().unwrap();
        assert_eq!(merkle_blob.key_to_index.len(), 1);
    }

    #[rstest]
    fn test_insert_choosing_side(
        #[values(Side::Left, Side::Right)] side: Side,
        #[values(1, 2)] pre_count: usize,
    ) {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let mut last_key: KvId = 0;
        for i in 1..=pre_count {
            let key: KvId = i as KvId;
            // open_dot(&mut merkle_blob.to_dot().set_note("empty"));
            merkle_blob
                .insert(key, key, &sha256_num(key), InsertLocation::Auto)
                .unwrap();
            last_key = key;
        }

        let key_value_id: KvId = pre_count as KvId + 1;
        // open_dot(&mut merkle_blob.to_dot().set_note("first after"));
        merkle_blob
            .insert(
                key_value_id,
                key_value_id,
                &sha256_num(key_value_id),
                InsertLocation::Leaf {
                    index: merkle_blob.key_to_index[&last_key],
                    side: side.clone(),
                },
            )
            .unwrap();
        // open_dot(&mut merkle_blob.to_dot().set_note("first after"));

        let sibling = merkle_blob
            .get_node(merkle_blob.key_to_index[&last_key])
            .unwrap();
        let parent = merkle_blob.get_node(sibling.parent.unwrap()).unwrap();
        let NodeSpecific::Internal { left, right } = parent.specific else {
            panic!()
        };

        let NodeSpecific::Leaf { key: left_key, .. } = merkle_blob.get_node(left).unwrap().specific
        else {
            panic!()
        };
        let NodeSpecific::Leaf { key: right_key, .. } =
            merkle_blob.get_node(right).unwrap().specific
        else {
            panic!()
        };

        let expected_keys: [KvId; 2] = match side {
            Side::Left => [pre_count as KvId + 1, pre_count as KvId],
            Side::Right => [pre_count as KvId, pre_count as KvId + 1],
        };
        assert_eq!([left_key, right_key], expected_keys);

        merkle_blob.check().unwrap();
    }

    #[test]
    fn test_delete_last() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_id: KvId = 1;
        // open_dot(&mut merkle_blob.to_dot().set_note("empty"));
        merkle_blob
            .insert(
                key_value_id,
                key_value_id,
                &sha256_num(key_value_id),
                InsertLocation::Auto,
            )
            .unwrap();
        // open_dot(&mut merkle_blob.to_dot().set_note("first after"));
        merkle_blob.check().unwrap();

        merkle_blob.delete(key_value_id).unwrap();

        merkle_blob.check().unwrap();
        assert_eq!(merkle_blob.key_to_index.len(), 0);
    }

    #[rstest]
    // TODO: does this mut allow modifying the fixture value as used by other tests?
    fn test_delete_frees_index(mut small_blob: MerkleBlob) {
        let key = 0x0001_0203_0405_0607;
        let index = small_blob.key_to_index[&key];
        small_blob.delete(key).unwrap();

        assert_eq!(small_blob.free_indexes, vec![index, 2]);
    }

    #[rstest]
    // TODO: does this mut allow modifying the fixture value as used by other tests?
    fn test_get_new_index_with_free_index(mut small_blob: MerkleBlob) {
        let key = 0x0001_0203_0405_0607;
        let _ = small_blob.key_to_index[&key];
        small_blob.delete(key).unwrap();

        // NOTE: both 1 and 2 are free per test_delete_frees_index
        assert_eq!(small_blob.get_new_index(), 2);
    }

    #[rstest]
    fn test_dump_small_blob_bytes(small_blob: MerkleBlob) {
        println!("{}", hex::encode(small_blob.blob));
    }

    #[test]
    #[should_panic(expected = "unknown NodeType value: 2")]
    fn test_node_type_from_u8_invalid() {
        let _ = NodeType::from_u8(2);
    }

    #[test]
    fn test_node_metadata_dirty_from_bytes_invalid() {
        NodeMetadata::dirty_from_bytes([0, 2]).expect_err("invalid value should fail");
    }

    #[test]
    #[should_panic(expected = "unable to get sibling index from a leaf")]
    fn test_node_specific_sibling_index_panics_for_leaf() {
        let leaf = NodeSpecific::Leaf { key: 0, value: 0 };
        leaf.sibling_index(0);
    }

    #[test]
    #[should_panic(expected = "index not a child: 2")]
    fn test_node_specific_sibling_index_panics_for_unknown_sibling() {
        let node = NodeSpecific::Internal { left: 0, right: 1 };
        node.sibling_index(2);
    }

    #[rstest]
    fn test_get_free_indexes(small_blob: MerkleBlob) {
        let mut blob = small_blob.blob.clone();
        let expected_free_index = (blob.len() / BLOCK_SIZE) as TreeIndex;
        blob.extend_from_slice(&[0; BLOCK_SIZE]);
        assert_eq!(get_free_indexes(&blob), [expected_free_index]);
    }

    #[test]
    fn test_merkle_blob_new_errs_for_nonmultiple_of_block_length() {
        MerkleBlob::new(vec![1]).expect_err("invalid length should fail");
    }

    #[rstest]
    fn test_upsert_inserts(small_blob: MerkleBlob) {
        let key = 1234;
        assert!(!small_blob.key_to_index.contains_key(&key));
        let value = 5678;

        let mut insert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        insert_blob
            .insert(key, value, &sha256_num(key), InsertLocation::Auto)
            .unwrap();
        // open_dot(&mut insert_blob.to_dot().set_note("first after"));

        let mut upsert_blob = MerkleBlob::new(small_blob.blob.clone()).unwrap();
        upsert_blob.upsert(key, value, &sha256_num(key)).unwrap();
        // open_dot(&mut upsert_blob.to_dot().set_note("first after"));

        assert_eq!(insert_blob.blob, upsert_blob.blob);
    }

    #[rstest]
    // TODO: does this mut allow modifying the fixture value as used by other tests?
    fn test_upsert_upserts(mut small_blob: MerkleBlob) {
        let before_blocks = small_blob.iter().collect::<Vec<_>>();
        let (key, index) = small_blob.key_to_index.iter().next().unwrap();
        let node = small_blob.get_node(*index).unwrap();
        let NodeSpecific::Leaf {
            key: original_key,
            value: original_value,
            ..
        } = node.specific
        else {
            panic!()
        };
        let new_value = original_value + 1;

        small_blob.upsert(*key, new_value, &node.hash).unwrap();

        let after_blocks = small_blob.iter().collect::<Vec<_>>();

        assert_eq!(before_blocks.len(), after_blocks.len());
        for ((before_index, before), (after_index, after)) in zip(before_blocks, after_blocks) {
            assert_eq!(before.node.parent, after.node.parent);
            assert_eq!(before_index, after_index);
            let NodeSpecific::Leaf {
                key: before_key,
                value: before_value,
            } = before.node.specific
            else {
                assert_eq!(before.node.specific, after.node.specific);
                continue;
            };
            let NodeSpecific::Leaf {
                key: after_key,
                value: after_value,
            } = after.node.specific
            else {
                panic!()
            };
            assert_eq!(before_key, after_key);
            if before_key == original_key {
                assert_eq!(after_value, new_value);
            } else {
                assert_eq!(before_value, after_value);
            }
        }
    }
}
