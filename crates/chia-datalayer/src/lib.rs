#[cfg(feature = "py-bindings")]
use pyo3::{buffer::PyBuffer, pyclass, pymethods, PyResult};

use clvmr::sha2::Sha256;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::iter::IntoIterator;
use std::ops::Range;

// TODO: clearly shouldn't be hard coded
const METADATA_SIZE: usize = 2;
// TODO: clearly shouldn't be hard coded
const DATA_SIZE: usize = 44;
const BLOCK_SIZE: usize = METADATA_SIZE + DATA_SIZE;

type TreeIndex = u32;
type Parent = Option<TreeIndex>;
type Hash = [u8; 32];
type BlockBytes = [u8; BLOCK_SIZE];
type KvId = u64;

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

fn internal_hash(left_hash: Hash, right_hash: Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(b"\x02");
    hasher.update(left_hash);
    hasher.update(right_hash);

    hasher.finalize()
}

pub struct DotLines {
    nodes: Vec<String>,
    connections: Vec<String>,
    pair_boxes: Vec<String>,
}

impl Default for DotLines {
    fn default() -> Self {
        Self::new()
    }
}

impl DotLines {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            connections: vec![],
            pair_boxes: vec![],
        }
    }

    pub fn push(&mut self, mut other: DotLines) {
        self.nodes.append(&mut other.nodes);
        self.connections.append(&mut other.connections);
        self.pair_boxes.append(&mut other.pair_boxes);
    }

    pub fn dump(&mut self) -> String {
        // TODO: consuming itself, secretly
        let mut result = vec!["digraph {".to_string()];
        result.append(&mut self.nodes);
        result.append(&mut self.connections);
        result.append(&mut self.pair_boxes);
        result.push("}".to_string());

        result.join("\n")
    }
}

const NULL_PARENT: TreeIndex = 0xffff_ffffu32;

#[derive(Debug, PartialEq)]
pub struct NodeMetadata {
    pub node_type: NodeType,
    pub dirty: bool,
}

impl NodeMetadata {
    pub fn from_bytes(blob: [u8; METADATA_SIZE]) -> Result<Self, String> {
        // TODO: could save 1-2% of tree space by packing (and maybe don't do that)
        // TODO: identify some useful structured serialization tooling we use
        Ok(Self {
            node_type: Self::node_type_from_bytes(blob)?,
            dirty: Self::dirty_from_bytes(blob)?,
        })
    }

    pub fn to_bytes(&self) -> [u8; METADATA_SIZE] {
        [self.node_type.to_u8(), u8::from(self.dirty)]
    }

    pub fn node_type_from_bytes(blob: [u8; METADATA_SIZE]) -> Result<NodeType, String> {
        NodeType::from_u8(blob[0])
    }

    pub fn dirty_from_bytes(blob: [u8; METADATA_SIZE]) -> Result<bool, String> {
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
    // TODO: kinda feels questionable having it be aware of its own location
    index: TreeIndex,
}

#[derive(Debug, PartialEq)]
pub enum NodeSpecific {
    Internal { left: TreeIndex, right: TreeIndex },
    Leaf { key_value: KvId },
}

impl NodeSpecific {
    pub fn sibling_index(&self, index: TreeIndex) -> TreeIndex {
        let NodeSpecific::Internal { right, left } = self else {
            panic!()
        };

        match index {
            x if (x == *right) => *left,
            x if (x == *left) => *right,
            _ => panic!(),
        }
    }
}

const PARENT_RANGE: Range<usize> = 0..4;
const LEFT_RANGE: Range<usize> = 4..8;
const RIGHT_RANGE: Range<usize> = 8..12;
const KEY_VALUE_RANGE: Range<usize> = 4..12;
// TODO: move the common parts to the beginning of the serialization?
const HASH_RANGE: Range<usize> = 12..44;

impl Node {
    // fn discriminant(&self) -> u8 {
    //     unsafe { *(self as *const Self as *const u8) }
    // }

    pub fn from_bytes(
        metadata: &NodeMetadata,
        index: TreeIndex,
        blob: [u8; DATA_SIZE],
    ) -> Result<Self, String> {
        // TODO: add Err results
        Ok(Self {
            parent: Self::parent_from_bytes(&blob)?,
            index,
            hash: <[u8; 32]>::try_from(&blob[HASH_RANGE]).unwrap(),
            specific: match metadata.node_type {
                NodeType::Internal => NodeSpecific::Internal {
                    left: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[LEFT_RANGE]).unwrap()),
                    right: TreeIndex::from_be_bytes(
                        <[u8; 4]>::try_from(&blob[RIGHT_RANGE]).unwrap(),
                    ),
                },
                NodeType::Leaf => NodeSpecific::Leaf {
                    // TODO: this try from really right?
                    key_value: KvId::from_be_bytes(
                        <[u8; 8]>::try_from(&blob[KEY_VALUE_RANGE]).unwrap(),
                    ),
                },
            },
        })
    }

    fn parent_from_bytes(blob: &[u8; DATA_SIZE]) -> Result<Parent, String> {
        // TODO: a little setup here for pre-optimization to allow walking parents without processing entire nodes
        let parent_integer = TreeIndex::from_be_bytes(
            <[u8; 4]>::try_from(&blob[PARENT_RANGE])
                .map_err(|e| format!("data blob wrong size: {e}"))?,
        );
        match parent_integer {
            NULL_PARENT => Ok(None),
            _ => Ok(Some(parent_integer)),
        }
    }
    pub fn to_bytes(&self) -> [u8; DATA_SIZE] {
        let mut blob: Vec<u8> = Vec::new();
        match self {
            Node {
                parent,
                specific: NodeSpecific::Internal { left, right },
                hash,
                index: _,
            } => {
                let parent_integer = match parent {
                    None => NULL_PARENT,
                    Some(parent) => *parent,
                };
                blob.extend(parent_integer.to_be_bytes());
                blob.extend(left.to_be_bytes());
                blob.extend(right.to_be_bytes());
                blob.extend(hash);
            }
            Node {
                parent,
                specific: NodeSpecific::Leaf { key_value },
                hash,
                index: _,
            } => {
                let parent_integer = match parent {
                    None => NULL_PARENT,
                    Some(parent) => *parent,
                };
                blob.extend(parent_integer.to_be_bytes());
                blob.extend(key_value.to_be_bytes());
                blob.extend(hash);
            }
        }

        blob.try_into().unwrap()
    }

    // TODO: yes i know i'm trying to write this code in a non-rusty way and i need to stop that
    pub fn key_value(&self) -> KvId {
        let NodeSpecific::Leaf { key_value } = self.specific else {
            panic!()
        };

        key_value
    }

    pub fn to_dot(&self) -> DotLines {
        let index = self.index;
        match self.specific {
            NodeSpecific::Internal {left, right} => DotLines{
                nodes: vec![
                    format!("node_{index} [label=\"{index}\"]"),
                ],
                connections: vec![
                    format!("node_{index} -> node_{left};"),
                    format!("node_{index} -> node_{right};"),
                    // TODO: can this be done without introducing a blank line?
                    match self.parent{
                        Some(parent) => format!("node_{index} -> node_{parent};"),
                        None => String::new(),
                    },
                ],
                pair_boxes: vec![
                    format!("node [shape = box]; {{rank = same; node_{left}->node_{right}[style=invis]; rankdir = LR}}"),
                ]
            },
            NodeSpecific::Leaf {key_value} => DotLines{
                nodes: vec![
                    format!("node_{index} [shape=box, label=\"{index}\\nkey_value: {key_value}\"];"),
                ],
                connections: vec![
                    // TODO: dedupe with above
                    match self.parent{
                        Some(parent) => format!("node_{index} -> node_{parent};"),
                        None => String::new(),
                    },
                ],
                pair_boxes: vec![],
            },
        }
    }
}

// TODO: does not enforce matching metadata node type and node enumeration type
pub struct Block {
    metadata: NodeMetadata,
    node: Node,
}

impl Block {
    pub fn to_bytes(&self) -> BlockBytes {
        let mut blob: BlockBytes = [0; BLOCK_SIZE];
        blob[..METADATA_SIZE].copy_from_slice(&self.metadata.to_bytes());
        blob[METADATA_SIZE..].copy_from_slice(&self.node.to_bytes());

        blob
    }

    pub fn from_bytes(blob: BlockBytes, index: TreeIndex) -> Result<Block, String> {
        // TODO: handle invalid indexes?
        // TODO: handle overflows?
        let metadata_blob: [u8; METADATA_SIZE] = blob
            .get(..METADATA_SIZE)
            .ok_or(format!("metadata blob out of bounds: {}", blob.len(),))?
            .try_into()
            .map_err(|e| format!("metadata blob wrong size: {e}"))?;
        let data_blob: [u8; DATA_SIZE] = blob
            .get(METADATA_SIZE..)
            .ok_or("data blob out of bounds".to_string())?
            .try_into()
            .map_err(|e| format!("data blob wrong size: {e}"))?;
        let metadata = match NodeMetadata::from_bytes(metadata_blob) {
            Ok(metadata) => metadata,
            Err(message) => return Err(format!("failed loading metadata: {message})")),
        };
        Ok(match Node::from_bytes(&metadata, index, data_blob) {
            Ok(node) => Block { metadata, node },
            Err(message) => return Err(format!("failed loading node: {message}")),
        })
    }

    fn range(index: TreeIndex) -> Range<usize> {
        let metadata_start = index as usize * BLOCK_SIZE;
        let data_start = metadata_start + METADATA_SIZE;
        let end = data_start + DATA_SIZE;

        // let range = metadata_start..end;
        // // checking range validity
        // self.blob.get(range.clone()).unwrap();
        //
        // range
        metadata_start..end
    }
}

// TODO: once error handling is well defined, remove allow and handle warning
#[allow(clippy::unnecessary_wraps)]
fn get_free_indexes(blob: &[u8]) -> Result<Vec<TreeIndex>, String> {
    let index_count = blob.len() / BLOCK_SIZE;

    if index_count == 0 {
        return Ok(vec![]);
    }

    let mut seen_indexes: Vec<bool> = vec![false; index_count];

    for block in MerkleBlobIterator::new(blob) {
        seen_indexes[block.node.index as usize] = true;
    }

    let mut free_indexes: Vec<TreeIndex> = vec![];
    for (index, seen) in seen_indexes.iter().enumerate() {
        if !seen {
            free_indexes.push(index as TreeIndex);
        }
    }

    Ok(free_indexes)
}

// TODO: once error handling is well defined, remove allow and handle warning
#[allow(clippy::unnecessary_wraps)]
fn get_keys_values_indexes(blob: &[u8]) -> Result<HashMap<KvId, TreeIndex>, String> {
    let index_count = blob.len() / BLOCK_SIZE;

    let mut kv_to_index: HashMap<KvId, TreeIndex> = HashMap::default();

    if index_count == 0 {
        return Ok(kv_to_index);
    }

    for block in MerkleBlobIterator::new(blob) {
        match block.node.specific {
            NodeSpecific::Leaf { key_value } => {
                kv_to_index.insert(key_value, block.node.index);
            }
            NodeSpecific::Internal { .. } => (),
        }
    }

    Ok(kv_to_index)
}

#[cfg_attr(feature = "py-bindings", pyclass(name = "MerkleBlob"))]
pub struct MerkleBlob {
    blob: Vec<u8>,
    free_indexes: Vec<TreeIndex>,
    kv_to_index: HashMap<KvId, TreeIndex>,
    // TODO: maybe name it next_index_to_allocate
    last_allocated_index: TreeIndex,
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
        let free_indexes = get_free_indexes(&blob).unwrap();
        let kv_to_index = get_keys_values_indexes(&blob).unwrap();

        Ok(Self {
            blob,
            free_indexes,
            kv_to_index,
            last_allocated_index: block_count as TreeIndex,
        })
    }

    pub fn insert(&mut self, key_value: KvId, hash: Hash) -> Result<(), String> {
        // TODO: what about only unused providing a blob length?
        if self.blob.is_empty() {
            self.insert_first(key_value, hash);
        }

        // TODO: make this a parameter so we have one insert call where you specify the location
        let old_leaf = self.get_random_leaf_node_from_bytes(Vec::from(key_value.to_be_bytes()))?;
        let internal_node_hash = internal_hash(old_leaf.hash, hash);

        if self.kv_to_index.len() == 1 {
            self.insert_second(key_value, hash, &old_leaf, internal_node_hash);
            return Ok(());
        }

        self.insert_third_or_later(key_value, hash, &old_leaf, internal_node_hash)
    }

    fn insert_first(&mut self, key_value: KvId, hash: Hash) {
        let new_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node {
                parent: None,
                specific: NodeSpecific::Leaf { key_value },
                hash,
                index: 0,
            },
        };

        self.blob.extend(new_leaf_block.to_bytes());

        self.kv_to_index.insert(key_value, 0);
        self.free_indexes.clear();
        self.last_allocated_index = 1;
    }

    fn insert_second(
        &mut self,
        key_value: KvId,
        hash: Hash,
        old_leaf: &Node,
        internal_node_hash: Hash,
    ) {
        self.blob.clear();

        let new_internal_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node {
                parent: None,
                specific: NodeSpecific::Internal { left: 1, right: 2 },
                hash: internal_node_hash,
                index: 0,
            },
        };

        self.blob.extend(new_internal_block.to_bytes());

        let left_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node {
                parent: Some(0),
                specific: NodeSpecific::Leaf {
                    key_value: old_leaf.key_value(),
                },
                hash: old_leaf.hash,
                index: 1,
            },
        };
        self.blob.extend(left_leaf_block.to_bytes());
        self.kv_to_index
            .insert(left_leaf_block.node.key_value(), left_leaf_block.node.index);

        let right_leaf_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: Node {
                parent: Some(0),
                specific: NodeSpecific::Leaf { key_value },
                hash,
                index: 2,
            },
        };
        self.blob.extend(right_leaf_block.to_bytes());
        self.kv_to_index.insert(
            right_leaf_block.node.key_value(),
            right_leaf_block.node.index,
        );

        self.free_indexes.clear();
        self.last_allocated_index = 3;
    }

    fn insert_third_or_later(
        &mut self,
        key_value: KvId,
        hash: Hash,
        old_leaf: &Node,
        internal_node_hash: Hash,
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
                specific: NodeSpecific::Leaf { key_value },
                hash,
                index: new_leaf_index,
            },
        };
        self.insert_entry_to_blob(new_leaf_index, new_leaf_block.to_bytes())?;

        let new_internal_block = Block {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: Node {
                parent: old_leaf.parent,
                specific: NodeSpecific::Internal {
                    left: old_leaf.index,
                    right: new_leaf_index,
                },
                hash: internal_node_hash,
                index: new_internal_node_index,
            },
        };
        self.insert_entry_to_blob(new_internal_node_index, new_internal_block.to_bytes())?;

        let Some(old_parent_index) = old_leaf.parent else {
            panic!("{key_value:?} {hash:?}")
        };

        let mut block = Block::from_bytes(
            self.get_block_bytes(old_leaf.index)?,
            new_internal_node_index,
        )?;
        block.node.parent = Some(new_internal_node_index);
        self.insert_entry_to_blob(old_leaf.index, block.to_bytes())?;

        let mut old_parent_block =
            Block::from_bytes(self.get_block_bytes(old_parent_index)?, old_parent_index)?;
        match old_parent_block.node.specific {
            NodeSpecific::Internal {
                ref mut left,
                ref mut right,
                ..
            } => {
                if old_leaf.index == *left {
                    *left = new_internal_node_index;
                } else if old_leaf.index == *right {
                    *right = new_internal_node_index;
                } else {
                    panic!();
                }
            }
            NodeSpecific::Leaf { .. } => panic!(),
        }
        self.insert_entry_to_blob(old_parent_index, old_parent_block.to_bytes())?;

        self.mark_lineage_as_dirty(old_parent_index)?;
        self.kv_to_index.insert(key_value, new_leaf_index);

        Ok(())
    }

    pub fn delete(&mut self, key_value: KvId) -> Result<(), String> {
        let leaf_index = *self.kv_to_index.get(&key_value).unwrap();
        let leaf = self.get_node(leaf_index).unwrap();

        match leaf.specific {
            // TODO: blech
            NodeSpecific::Leaf { .. } => (),
            NodeSpecific::Internal { .. } => panic!(),
        };
        self.kv_to_index.remove(&key_value);

        let Some(parent_index) = leaf.parent else {
            self.free_indexes.clear();
            self.last_allocated_index = 0;
            self.blob.clear();
            return Ok(());
        };

        self.free_indexes.push(leaf_index);
        let parent = self.get_node(parent_index).unwrap();
        // TODO: kinda implicit that we 'check' that parent is internal inside .sibling_index()
        let sibling_index = parent.specific.sibling_index(leaf_index);
        let mut sibling_block = self.get_block(sibling_index)?;

        let Some(grandparent_index) = parent.parent else {
            sibling_block.metadata.dirty = true;
            sibling_block.node.parent = None;
            self.insert_entry_to_blob(0, sibling_block.to_bytes())?;

            match sibling_block.node.specific {
                NodeSpecific::Leaf { key_value } => {
                    self.kv_to_index.insert(key_value, 0);
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
        let mut grandparent_block = self.get_block(grandparent_index).unwrap();

        sibling_block.node.parent = Some(grandparent_index);
        self.insert_entry_to_blob(sibling_index, sibling_block.to_bytes())?;

        match grandparent_block.node.specific {
            NodeSpecific::Internal {
                ref mut left,
                ref mut right,
                ..
            } => match parent_index {
                x if x == *left => *left = sibling_index,
                x if x == *right => *right = sibling_index,
                _ => panic!(),
            },
            NodeSpecific::Leaf { .. } => panic!(),
        };
        self.insert_entry_to_blob(grandparent_index, grandparent_block.to_bytes())?;

        self.mark_lineage_as_dirty(grandparent_index)?;

        Ok(())
    }

    // fn upsert(&self, old_key_value: KvId, new_key_value: KvId, new_hash: Hash) -> Result<(), String> {
    //     if old_key_value
    // }

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
            let mut block = Block::from_bytes(self.get_block_bytes(this_index)?, this_index)?;

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
                self.last_allocated_index += 1;
                self.last_allocated_index - 1
            }
            Some(new_index) => new_index,
        }
    }

    fn get_random_leaf_node_from_bytes(&self, seed_bytes: Vec<u8>) -> Result<Node, String> {
        let mut hasher = Sha256::new();
        hasher.update(seed_bytes);
        let seed: Hash = hasher.finalize();

        let mut node = self.get_node(0)?;
        for byte in seed {
            for bit in 0..8 {
                match node.specific {
                    NodeSpecific::Leaf { .. } => return Ok(node),
                    NodeSpecific::Internal { left, right, .. } => {
                        let next: TreeIndex = if byte & (1 << bit) != 0 { left } else { right };
                        node = self.get_node(next)?;
                    }
                }
            }
        }

        Err("failed to find a node".to_string())
    }

    fn insert_entry_to_blob(
        &mut self,
        index: TreeIndex,
        block_bytes: BlockBytes,
    ) -> Result<(), String> {
        assert_eq!(self.blob.len() % BLOCK_SIZE, 0);
        let extend_index = (self.blob.len() / BLOCK_SIZE) as TreeIndex;
        match index.cmp(&extend_index) {
            Ordering::Greater => return Err(format!("index out of range: {index}")),
            Ordering::Equal => self.blob.extend_from_slice(&block_bytes),
            Ordering::Less => {
                self.blob[Block::range(index)].copy_from_slice(&block_bytes);
            }
        }

        Ok(())
    }

    fn get_block(&self, index: TreeIndex) -> Result<Block, String> {
        Block::from_bytes(self.get_block_bytes(index)?, index)
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
            .get(Block::range(index))
            .ok_or(format!("index out of bounds: {index}"))?
            .try_into()
            .map_err(|e| format!("failed getting block {index}: {e}"))
    }

    pub fn get_node(&self, index: TreeIndex) -> Result<Node, String> {
        // TODO: use Block::from_bytes()
        // TODO: handle invalid indexes?
        // TODO: handle overflows?
        let block = self.get_block_bytes(index)?;
        let metadata_blob: [u8; METADATA_SIZE] = block
            .get(..METADATA_SIZE)
            .ok_or(format!("metadata blob out of bounds: {}", block.len(),))?
            .try_into()
            .map_err(|e| format!("metadata blob wrong size: {e}"))?;
        let data_blob: [u8; DATA_SIZE] = block
            .get(METADATA_SIZE..)
            .ok_or("data blob out of bounds".to_string())?
            .try_into()
            .map_err(|e| format!("data blob wrong size: {e}"))?;
        let metadata = match NodeMetadata::from_bytes(metadata_blob) {
            Ok(metadata) => metadata,
            Err(message) => return Err(format!("failed loading metadata: {message})")),
        };
        Ok(match Node::from_bytes(&metadata, index, data_blob) {
            Ok(node) => node,
            Err(message) => return Err(format!("failed loading node: {message}")),
        })
    }

    pub fn get_parent_index(&self, index: TreeIndex) -> Result<Parent, String> {
        let block = self.get_block_bytes(index).unwrap();

        Node::parent_from_bytes(
            block[METADATA_SIZE..]
                .try_into()
                .map_err(|e| format!("data blob wrong size: {e}"))?,
        )
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
            let block = self.get_block_bytes(this_index)?;
            next_index = Node::parent_from_bytes(block[METADATA_SIZE..].try_into().unwrap())?;
        }

        Ok(lineage)
    }

    pub fn to_dot(&self) -> DotLines {
        let mut result = DotLines::new();
        for block in self {
            result.push(block.node.to_dot());
        }

        result
    }

    pub fn iter(&self) -> MerkleBlobIterator<'_> {
        <&Self as IntoIterator>::into_iter(self)
    }

    pub fn calculate_lazy_hashes(&mut self) {
        // TODO: really want a truncated traversal, not filter
        // TODO: yeah, storing the whole set of blocks via collect is not great
        for mut block in self
            .iter()
            .filter(|block| block.metadata.dirty)
            .collect::<Vec<_>>()
        {
            match block.node.specific {
                NodeSpecific::Leaf { .. } => panic!("leaves should not be dirty"),
                NodeSpecific::Internal { left, right, .. } => {
                    // TODO: obviously inefficient to re-get/deserialize these blocks inside
                    //       an iteration that's already doing that
                    let left = self.get_block(left).unwrap();
                    let right = self.get_block(right).unwrap();
                    // TODO: wrap this up in Block maybe? just to have 'control' of dirty being 'accurate'
                    block.node.hash = internal_hash(left.node.hash, right.node.hash);
                    block.metadata.dirty = false;
                    self.insert_entry_to_blob(block.node.index, block.to_bytes())
                        .unwrap();
                }
            }
        }
    }
}

impl<'a> IntoIterator for &'a MerkleBlob {
    // TODO: review efficiency in whatever use cases we end up with, vs Item = Node etc
    type Item = Block;
    type IntoIter = MerkleBlobIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        // TODO: review types around this to avoid copying
        MerkleBlobIterator::new(&self.blob[..])
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
    pub fn py_insert(&mut self, key_value: KvId, hash: Hash) -> PyResult<()> {
        // TODO: consider the error
        self.insert(key_value, hash).unwrap();

        Ok(())
    }

    #[pyo3(name = "delete")]
    pub fn py_delete(&mut self, key_value: KvId) -> PyResult<()> {
        // TODO: consider the error
        self.delete(key_value).unwrap();

        Ok(())
    }

    #[pyo3(name = "__len__")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.blob.len())
    }
}

struct MerkleBlobIteratorItem {
    visited: bool,
    index: TreeIndex,
}

pub struct MerkleBlobIterator<'a> {
    blob: &'a [u8],
    deque: VecDeque<MerkleBlobIteratorItem>,
}

impl<'a> MerkleBlobIterator<'a> {
    fn new(blob: &'a [u8]) -> Self {
        let mut deque = VecDeque::new();
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(MerkleBlobIteratorItem {
                visited: false,
                index: 0,
            });
        }

        Self { blob, deque }
    }
}

impl Iterator for MerkleBlobIterator<'_> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, children before parents

        loop {
            let item = self.deque.pop_front()?;
            let block_bytes: BlockBytes = self.blob[Block::range(item.index)].try_into().unwrap();
            let block = Block::from_bytes(block_bytes, item.index).unwrap();

            match block.node.specific {
                NodeSpecific::Leaf { .. } => return Some(block),
                NodeSpecific::Internal { left, right } => {
                    if item.visited {
                        return Some(block);
                    };

                    self.deque.push_front(MerkleBlobIteratorItem {
                        visited: true,
                        index: item.index,
                    });
                    self.deque.push_front(MerkleBlobIteratorItem {
                        visited: false,
                        index: right,
                    });
                    self.deque.push_front(MerkleBlobIteratorItem {
                        visited: false,
                        index: left,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use rstest::rstest;
    use std::time::{Duration, Instant};

    const EXAMPLE_BLOB: [u8; 138] = hex!("0001ffffffff00000001000000020c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000000405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000001415161718191a1b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b");
    const HASH: Hash = [
        12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
        35, 36, 37, 38, 39, 40, 41, 42, 43,
    ];

    const EXAMPLE_ROOT: Node = Node {
        parent: None,
        specific: NodeSpecific::Internal { left: 1, right: 2 },
        hash: HASH,
        index: 0,
    };
    const EXAMPLE_ROOT_METADATA: NodeMetadata = NodeMetadata {
        node_type: NodeType::Internal,
        dirty: true,
    };
    const EXAMPLE_LEFT_LEAF: Node = Node {
        parent: Some(0),
        specific: NodeSpecific::Leaf {
            key_value: 0x0405_0607_0809_0A0B,
        },
        hash: HASH,
        index: 1,
    };
    const EXAMPLE_LEFT_LEAF_METADATA: NodeMetadata = NodeMetadata {
        node_type: NodeType::Leaf,
        dirty: false,
    };
    const EXAMPLE_RIGHT_LEAF: Node = Node {
        parent: Some(0),
        specific: NodeSpecific::Leaf {
            key_value: 0x1415_1617_1819_1A1B,
        },
        hash: HASH,
        index: 2,
    };
    const EXAMPLE_RIGHT_LEAF_METADATA: NodeMetadata = NodeMetadata {
        node_type: NodeType::Leaf,
        dirty: false,
    };

    fn example_merkle_blob() -> MerkleBlob {
        MerkleBlob::new(Vec::from(EXAMPLE_BLOB)).unwrap()
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
            internal_hash(left, right),
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

    #[test]
    fn test_load_a_python_dump() {
        let merkle_blob = example_merkle_blob();
        merkle_blob.get_node(0).unwrap();
    }

    #[test]
    fn test_get_lineage() {
        let merkle_blob = example_merkle_blob();
        let lineage = merkle_blob.get_lineage(2).unwrap();
        for node in &lineage {
            println!("{node:?}");
        }
        assert_eq!(lineage.len(), 2);
        let last_node = lineage.last().unwrap();
        assert_eq!(last_node.parent, None);
    }

    #[test]
    fn test_get_random_leaf_node() {
        let merkle_blob = example_merkle_blob();
        let leaf = merkle_blob
            .get_random_leaf_node_from_bytes(vec![0; 8])
            .unwrap();
        assert_eq!(leaf.index, 1);
    }

    #[test]
    fn test_build_blob_and_read() {
        let mut blob: Vec<u8> = Vec::new();

        blob.extend(EXAMPLE_ROOT_METADATA.to_bytes());
        blob.extend(EXAMPLE_ROOT.to_bytes());
        blob.extend(EXAMPLE_LEFT_LEAF_METADATA.to_bytes());
        blob.extend(EXAMPLE_LEFT_LEAF.to_bytes());
        blob.extend(EXAMPLE_RIGHT_LEAF_METADATA.to_bytes());
        blob.extend(EXAMPLE_RIGHT_LEAF.to_bytes());

        assert_eq!(blob, Vec::from(EXAMPLE_BLOB));

        let merkle_blob = MerkleBlob::new(Vec::from(EXAMPLE_BLOB)).unwrap();

        assert_eq!(merkle_blob.get_node(0).unwrap(), EXAMPLE_ROOT);
        assert_eq!(merkle_blob.get_node(1).unwrap(), EXAMPLE_LEFT_LEAF);
        assert_eq!(merkle_blob.get_node(2).unwrap(), EXAMPLE_RIGHT_LEAF);
    }

    #[test]
    fn test_build_merkle() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        merkle_blob
            .insert(EXAMPLE_LEFT_LEAF.key_value(), EXAMPLE_LEFT_LEAF.hash)
            .unwrap();
        merkle_blob
            .insert(EXAMPLE_RIGHT_LEAF.key_value(), EXAMPLE_RIGHT_LEAF.hash)
            .unwrap();

        // TODO: just hacking here to compare with the ~wrong~ simplified reference
        let mut root = Block::from_bytes(merkle_blob.get_block_bytes(0).unwrap(), 0).unwrap();
        root.metadata.dirty = true;
        root.node.hash = HASH;
        assert_eq!(root.metadata.node_type, NodeType::Internal);
        merkle_blob
            .insert_entry_to_blob(0, root.to_bytes())
            .unwrap();

        assert_eq!(merkle_blob.blob, Vec::from(EXAMPLE_BLOB));
    }

    #[test]
    fn test_just_insert_a_bunch() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let mut total_time = Duration::new(0, 0);

        for i in 0..100_000 {
            let start = Instant::now();
            merkle_blob
                // TODO: yeah this hash is garbage
                .insert(i as KvId, HASH)
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

        merkle_blob.calculate_lazy_hashes();
    }

    #[test]
    fn test_delete() {
        const COUNT: usize = 10;
        let mut dots = vec![];

        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        let key_value_ids: [KvId; COUNT] = core::array::from_fn(|i| i as KvId);

        for key_value_id in key_value_ids {
            let mut hasher = Sha256::new();
            hasher.update(key_value_id.to_be_bytes());
            let hash: Hash = hasher.finalize();

            println!("inserting: {key_value_id}");
            merkle_blob.insert(key_value_id, hash).unwrap();
            dots.push(merkle_blob.to_dot().dump());
        }

        for key_value_id in key_value_ids {
            println!("deleting: {key_value_id}");
            merkle_blob.delete(key_value_id).unwrap();
            dots.push(merkle_blob.to_dot().dump());
        }

        // let mut key_value_ids: Vec<KvId> = vec![0; COUNT];
        //
        // for (i, key_value_id) in key_value_ids.iter_mut().enumerate() {
        //     *key_value_id = i as KvId;
        // }
        // for i in 0..100_000 {
        //     let start = Instant::now();
        //     merkle_blob
        //         // TODO: yeah this hash is garbage
        //         .insert(i as KvId, HASH)
        //         .unwrap();
        //     let end = Instant::now();
        //     total_time += end.duration_since(start);
        //
        //     // match i + 1 {
        //     //     2 => assert_eq!(merkle_blob.blob.len(), 3 * BLOCK_SIZE),
        //     //     3 => assert_eq!(merkle_blob.blob.len(), 5 * BLOCK_SIZE),
        //     //     _ => (),
        //     // }
        //
        //     // let file = fs::File::create(format!("/home/altendky/tmp/mbt/rs/{i:0>4}")).unwrap();
        //     // let mut file = io::LineWriter::new(file);
        //     // for block in merkle_blob.blob.chunks(BLOCK_SIZE) {
        //     //     let mut s = String::new();
        //     //     for byte in block {
        //     //         s.push_str(&format!("{:02x}", byte));
        //     //     }
        //     //     s.push_str("\n");
        //     //     file.write_all(s.as_bytes()).unwrap();
        //     // }
        //
        //     // fs::write(format!("/home/altendky/tmp/mbt/rs/{i:0>4}"), &merkle_blob.blob).unwrap();
        // }
        // // println!("{:?}", merkle_blob.blob)
        //
        // println!("total time: {total_time:?}");
        // // TODO: check, well...  something
    }
}
