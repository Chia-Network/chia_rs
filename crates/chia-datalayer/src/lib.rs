// use std::collections::HashMap;

use pyo3::buffer::PyBuffer;
#[cfg(feature = "py-bindings")]
use pyo3::{pyclass, pymethods, PyResult};

use clvmr::sha2::Sha256;
use std::cmp::Ordering;
use std::collections::HashMap;

// TODO: clearly shouldn't be hard coded
const METADATA_SIZE: usize = 2;
// TODO: clearly shouldn't be hard coded
const DATA_SIZE: usize = 44;
const BLOCK_SIZE: usize = METADATA_SIZE + DATA_SIZE;

type TreeIndex = u32;
// type Key = Vec<u8>;
type Hash = [u8; 32];
type Block = [u8; BLOCK_SIZE];
type KvId = u64;

#[derive(Debug, Hash, Eq, PartialEq)]
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
    // TODO: verify against original reference in blockchain
    let mut hasher = Sha256::new();
    hasher.update(b"\x02");
    hasher.update(left_hash);
    hasher.update(right_hash);

    hasher.finalize()
}

// TODO: probably bogus and overflowing or somesuch
const NULL_PARENT: TreeIndex = 0xffff_ffffu32; // 1 << (4 * 8) - 1;

// TODO: does not enforce matching metadata node type and node enumeration type
struct ParsedBlock {
    metadata: NodeMetadata,
    node: RawMerkleNode,
}

impl ParsedBlock {
    pub fn to_bytes(&self) -> [u8; BLOCK_SIZE] {
        let mut blob: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
        blob[..METADATA_SIZE].copy_from_slice(&self.metadata.to_bytes());
        blob[METADATA_SIZE..].copy_from_slice(&self.node.to_bytes());

        blob
    }

    pub fn from_bytes(blob: [u8; BLOCK_SIZE], index: TreeIndex) -> Result<ParsedBlock, String> {
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
        Ok(
            match RawMerkleNode::from_bytes(&metadata, index, data_blob) {
                Ok(node) => ParsedBlock { metadata, node },
                Err(message) => return Err(format!("failed loading raw node: {message}")),
            },
        )
    }
}
fn get_free_indexes(blob: &Vec<u8>) -> Result<Vec<TreeIndex>, String> {
    let index_count = blob.len() / BLOCK_SIZE;

    if index_count == 0 {
        return Ok(vec![]);
    }

    let mut seen_indexes: Vec<bool> = vec![false; index_count];
    let mut queue: Vec<TreeIndex> = vec![0];

    while queue.len() > 0 {
        let index: TreeIndex = queue.pop().unwrap();
        let offset = index as usize * BLOCK_SIZE;
        let block =
            ParsedBlock::from_bytes(blob[offset..offset + BLOCK_SIZE].try_into().unwrap(), index)?;
        seen_indexes[index as usize] = true;
        match block.node {
            RawMerkleNode::Internal { left, right, .. } => {
                queue.push(left);
                queue.push(right);
            }
            RawMerkleNode::Leaf { .. } => (),
        }
    }

    let mut free_indexes: Vec<TreeIndex> = vec![];
    for (index, seen) in seen_indexes.iter().enumerate() {
        if !seen {
            free_indexes.push(index as TreeIndex)
        }
    }

    Ok(free_indexes)
}

fn get_keys_values_indexes(blob: &Vec<u8>) -> Result<HashMap<KvId, TreeIndex>, String> {
    let index_count = blob.len() / BLOCK_SIZE;

    let mut kv_to_index: HashMap<KvId, TreeIndex> = HashMap::default();

    if index_count == 0 {
        return Ok(kv_to_index);
    }

    let mut queue: Vec<TreeIndex> = vec![0];

    while queue.len() > 0 {
        let index: TreeIndex = queue.pop().unwrap();
        let offset = index as usize * BLOCK_SIZE;
        let block =
            ParsedBlock::from_bytes(blob[offset..offset + BLOCK_SIZE].try_into().unwrap(), index)?;
        match block.node {
            RawMerkleNode::Leaf { key_value, .. } => {
                kv_to_index.insert(key_value, index);
            }
            RawMerkleNode::Internal { .. } => (),
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
        if self.blob.len() == 0 {
            let new_leaf_block = ParsedBlock {
                metadata: NodeMetadata {
                    node_type: NodeType::Leaf,
                    dirty: false,
                },
                node: RawMerkleNode::Leaf {
                    parent: NULL_PARENT,
                    key_value,
                    hash,
                    index: 0,
                },
            };

            self.blob.extend(new_leaf_block.to_bytes());

            self.kv_to_index.insert(key_value, 0);
            self.free_indexes.clear();
            self.last_allocated_index = 1;
            return Ok(());
        }
        let mut hasher = Sha256::new();
        hasher.update(key_value.to_be_bytes());
        let seed: Hash = hasher.finalize();
        let old_leaf = self.get_random_leaf_node(Vec::from(seed))?;
        let internal_node_hash = internal_hash(old_leaf.hash(), hash);

        if self.kv_to_index.len() == 1 {
            self.blob.clear();

            let new_internal_block = ParsedBlock {
                metadata: NodeMetadata {
                    node_type: NodeType::Internal,
                    dirty: false,
                },
                node: RawMerkleNode::Internal {
                    parent: NULL_PARENT,
                    left: 1,
                    right: 2,
                    hash: internal_node_hash,
                    index: 0,
                },
            };

            self.blob.extend(new_internal_block.to_bytes());

            let left_leaf_block = ParsedBlock {
                metadata: NodeMetadata {
                    node_type: NodeType::Leaf,
                    dirty: false,
                },
                node: RawMerkleNode::Leaf {
                    parent: 0,
                    key_value: old_leaf.key_value(),
                    hash: old_leaf.hash(),
                    index: 1,
                },
            };
            self.blob.extend(left_leaf_block.to_bytes());
            self.kv_to_index.insert(
                left_leaf_block.node.key_value(),
                left_leaf_block.node.index(),
            );

            let right_leaf_block = ParsedBlock {
                metadata: NodeMetadata {
                    node_type: NodeType::Leaf,
                    dirty: false,
                },
                node: RawMerkleNode::Leaf {
                    parent: 0,
                    key_value,
                    hash,
                    index: 2,
                },
            };
            self.blob.extend(right_leaf_block.to_bytes());
            self.kv_to_index.insert(
                right_leaf_block.node.key_value(),
                right_leaf_block.node.index(),
            );

            self.free_indexes.clear();
            self.last_allocated_index = 3;

            return Ok(());
        }

        let new_leaf_index = self.get_new_index();
        let new_internal_node_index = self.get_new_index();

        let new_leaf_block = ParsedBlock {
            metadata: NodeMetadata {
                node_type: NodeType::Leaf,
                dirty: false,
            },
            node: RawMerkleNode::Leaf {
                parent: new_internal_node_index,
                key_value,
                hash,
                index: new_leaf_index,
            },
        };
        self.insert_entry_to_blob(new_leaf_index, new_leaf_block.to_bytes())?;

        let new_internal_block = ParsedBlock {
            metadata: NodeMetadata {
                node_type: NodeType::Internal,
                dirty: false,
            },
            node: RawMerkleNode::Internal {
                parent: old_leaf.parent(),
                left: old_leaf.index(),
                right: new_leaf_index,
                hash: internal_node_hash,
                index: new_internal_node_index,
            },
        };
        self.insert_entry_to_blob(new_internal_node_index, new_internal_block.to_bytes())?;

        let old_parent_index = old_leaf.parent();
        assert!(
            old_parent_index != NULL_PARENT,
            "{}",
            format!("{key_value:?} {hash:?}")
        );

        let mut old_leaf_block =
            ParsedBlock::from_bytes(self.get_block(old_leaf.index())?, old_leaf.index())?;
        old_leaf_block.node.set_parent(new_internal_node_index);
        let offset = old_leaf_block.node.index() as usize * BLOCK_SIZE;
        self.blob[offset..offset + BLOCK_SIZE].copy_from_slice(&old_leaf_block.to_bytes());

        let mut old_parent_block =
            ParsedBlock::from_bytes(self.get_block(old_parent_index)?, old_parent_index)?;
        match old_parent_block.node {
            RawMerkleNode::Internal {
                ref mut left,
                ref mut right,
                ..
            } => {
                if old_leaf.index() == *left {
                    *left = new_internal_node_index;
                } else if old_leaf.index() == *right {
                    *right = new_internal_node_index;
                } else {
                    panic!();
                }
            }
            RawMerkleNode::Leaf { .. } => panic!(),
        }
        let offset = old_parent_index as usize * BLOCK_SIZE;
        self.blob[offset..offset + BLOCK_SIZE].copy_from_slice(&old_parent_block.to_bytes());

        self.mark_lineage_as_dirty(old_parent_index)?;
        self.kv_to_index.insert(key_value, new_internal_node_index);

        Ok(())
    }

    fn mark_lineage_as_dirty(&mut self, index: TreeIndex) -> Result<(), String> {
        let mut index = index;

        while index != NULL_PARENT {
            let mut block = ParsedBlock::from_bytes(self.get_block(index)?, index)?;
            block.metadata.dirty = true;
            let offset = index as usize * BLOCK_SIZE;
            self.blob[offset..offset + BLOCK_SIZE].copy_from_slice(&block.to_bytes());
            index = block.node.parent();
        }

        Ok(())
    }

    // fn update_entry(
    //     index: TreeIndex,
    //     parent: Option[TreeIndex],
    //     left: Option[TreeIndex],
    //     right: Option[TreeIndex],
    //     hash: Option[Hash],
    //     key_value: Option[KvId],
    // )
    fn get_new_index(&mut self) -> TreeIndex {
        match self.free_indexes.pop() {
            None => {
                self.last_allocated_index += 1;
                self.last_allocated_index - 1
            }
            Some(new_index) => new_index,
        }
    }

    fn get_random_leaf_node(&self, seed: Vec<u8>) -> Result<RawMerkleNode, String> {
        let mut node = self.get_raw_node(0)?;
        for byte in seed {
            for bit in 0..8 {
                match node {
                    RawMerkleNode::Leaf { .. } => return Ok(node),
                    RawMerkleNode::Internal { left, right, .. } => {
                        if byte & (1 << bit) != 0 {
                            node = self.get_raw_node(left)?;
                        } else {
                            node = self.get_raw_node(right)?;
                        }
                    }
                }
            }
        }

        Err("failed to find a node".to_string())
    }

    fn insert_entry_to_blob(&mut self, index: TreeIndex, block: Block) -> Result<(), String> {
        let extend_index = (self.blob.len() / BLOCK_SIZE) as TreeIndex;
        match index.cmp(&extend_index) {
            Ordering::Greater => return Err(format!("index out of range: {index}")),
            Ordering::Equal => self.blob.extend_from_slice(&block),
            Ordering::Less => {
                let start = index as usize * BLOCK_SIZE;
                self.blob[start..start + BLOCK_SIZE].copy_from_slice(&block);
            }
        }

        Ok(())
    }

    fn get_block(&self, index: TreeIndex) -> Result<Block, String> {
        let metadata_start = index as usize * BLOCK_SIZE;
        let data_start = metadata_start + METADATA_SIZE;
        let end = data_start + DATA_SIZE;

        self.blob
            .get(metadata_start..end)
            .ok_or(format!("index out of bounds: {index}"))?
            .try_into()
            .map_err(|e| format!("failed getting block {index}: {e}"))
    }

    pub fn get_raw_node(&self, index: TreeIndex) -> Result<RawMerkleNode, String> {
        // TODO: use ParsedBlock::from_bytes()
        // TODO: handle invalid indexes?
        // TODO: handle overflows?
        let block = self.get_block(index)?;
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
        Ok(
            match RawMerkleNode::from_bytes(&metadata, index, data_blob) {
                Ok(node) => node,
                Err(message) => return Err(format!("failed loading raw node: {message}")),
            },
        )
    }

    pub fn get_parent_index(&self, index: TreeIndex) -> Result<TreeIndex, String> {
        let block = self.get_block(index).unwrap();
        let node_type =
            NodeMetadata::node_type_from_bytes(block[..METADATA_SIZE].try_into().unwrap())?;

        RawMerkleNode::parent_from_bytes(&node_type, block[METADATA_SIZE..].try_into().unwrap())
    }

    pub fn get_lineage(&self, index: TreeIndex) -> Result<Vec<RawMerkleNode>, String> {
        let mut next_index = index;
        let mut lineage = vec![];
        loop {
            let node = self.get_raw_node(next_index)?;
            next_index = node.parent();
            lineage.push(node);

            if next_index == NULL_PARENT {
                return Ok(lineage);
            }
        }
    }

    pub fn get_lineage_indexes(&self, index: TreeIndex) -> Result<Vec<TreeIndex>, String> {
        // TODO: yep, this 'optimization' might be overkill, and should be speed compared regardless
        let mut next_index = index;
        let mut lineage = vec![];
        loop {
            lineage.push(next_index);
            let block = self.get_block(next_index)?;
            let node_type =
                NodeMetadata::node_type_from_bytes(block[..METADATA_SIZE].try_into().unwrap())?;
            next_index = RawMerkleNode::parent_from_bytes(
                &node_type,
                block[METADATA_SIZE..].try_into().unwrap(),
            )?;

            if next_index == NULL_PARENT {
                return Ok(lineage);
            }
        }
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl MerkleBlob {
    #[new]
    pub fn py_init(blob: PyBuffer<u8>) -> PyResult<Self> {
        if !blob.is_c_contiguous() {
            panic!("from_bytes() must be called with a contiguous buffer");
        }
        #[allow(unsafe_code)]
        let slice =
            unsafe { std::slice::from_raw_parts(blob.buf_ptr() as *const u8, blob.len_bytes()) };

        Ok(Self::new(Vec::from(slice)).unwrap())
    }

    // #[pyo3(name = "get_root")]
    // pub fn py_get_root<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
    //     ChiaToPython::to_python(&Bytes32::new(self.get_root()), py)
    // }

    #[pyo3(name = "insert")]
    pub fn py_insert(&mut self, key_value: KvId, hash: Hash) -> PyResult<()> {
        // TODO: consider the error
        // self.insert(key_value, hash).map_err(|_| PyValueError::new_err("yeppers"))
        self.insert(key_value, hash).unwrap();
        // self.insert(key_value, hash).map_err(|_| PyValueError::new_err("invalid key"))?;

        Ok(())
    }

    #[pyo3(name = "__len__")]
    pub fn py_len(&self) -> PyResult<usize> {
        Ok(self.blob.len())
    }
}

#[derive(Debug, PartialEq)]
pub enum RawMerkleNode {
    // Root {
    //     left: TreeIndex,
    //     right: TreeIndex,
    //     hash: Hash,
    //     // TODO: kinda feels questionable having it be aware of its own location
    //     // TODO: just always at zero?
    //     index: TreeIndex,
    // },
    Internal {
        parent: TreeIndex,
        left: TreeIndex,
        right: TreeIndex,
        hash: Hash,
        // TODO: kinda feels questionable having it be aware of its own location
        index: TreeIndex,
    },
    Leaf {
        parent: TreeIndex,
        key_value: KvId,
        hash: Hash,
        // TODO: kinda feels questionable having it be aware of its own location
        index: TreeIndex,
    },
}

impl RawMerkleNode {
    // fn discriminant(&self) -> u8 {
    //     unsafe { *(self as *const Self as *const u8) }
    // }

    pub fn from_bytes(
        metadata: &NodeMetadata,
        index: TreeIndex,
        blob: [u8; DATA_SIZE],
    ) -> Result<Self, String> {
        // TODO: add Err results
        let parent = Self::parent_from_bytes(&metadata.node_type, &blob)?;
        match metadata.node_type {
            NodeType::Internal => Ok(RawMerkleNode::Internal {
                // TODO: get these right
                parent,
                left: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[4..8]).unwrap()),
                right: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[8..12]).unwrap()),
                hash: <[u8; 32]>::try_from(&blob[12..44]).unwrap(),
                index,
            }),
            NodeType::Leaf => Ok(RawMerkleNode::Leaf {
                // TODO: this try from really right?
                parent,
                key_value: KvId::from_be_bytes(<[u8; 8]>::try_from(&blob[4..12]).unwrap()),
                hash: Hash::try_from(&blob[12..44]).unwrap(),
                index,
            }),
        }
    }

    fn parent_from_bytes(
        node_type: &NodeType,
        blob: &[u8; DATA_SIZE],
    ) -> Result<TreeIndex, String> {
        // TODO: a little setup here for pre-optimization to allow walking parents without processing entire nodes
        match node_type {
            NodeType::Internal => Ok(TreeIndex::from_be_bytes(
                <[u8; 4]>::try_from(&blob[0..4]).unwrap(),
            )),
            NodeType::Leaf => Ok(TreeIndex::from_be_bytes(
                <[u8; 4]>::try_from(&blob[0..4]).unwrap(),
            )),
        }
    }
    pub fn to_bytes(&self) -> [u8; DATA_SIZE] {
        let mut blob: Vec<u8> = Vec::new();
        match self {
            RawMerkleNode::Internal {
                parent,
                left,
                right,
                hash,
                index: _,
            } => {
                blob.extend(parent.to_be_bytes());
                blob.extend(left.to_be_bytes());
                blob.extend(right.to_be_bytes());
                blob.extend(hash);
            }
            RawMerkleNode::Leaf {
                parent,
                key_value,
                hash,
                index: _,
            } => {
                blob.extend(parent.to_be_bytes());
                blob.extend(key_value.to_be_bytes());
                blob.extend(hash);
            }
        }

        blob.try_into().unwrap()
    }

    pub fn parent(&self) -> TreeIndex {
        match self {
            RawMerkleNode::Internal { parent, .. } | RawMerkleNode::Leaf { parent, .. } => *parent,
        }
    }

    pub fn hash(&self) -> Hash {
        match self {
            RawMerkleNode::Internal { hash, .. } | RawMerkleNode::Leaf { hash, .. } => *hash,
        }
    }

    pub fn index(&self) -> TreeIndex {
        match self {
            RawMerkleNode::Internal { index, .. } | RawMerkleNode::Leaf { index, .. } => *index,
        }
    }

    pub fn set_parent(&mut self, p: TreeIndex) {
        match self {
            &mut RawMerkleNode::Internal { ref mut parent, .. }
            | RawMerkleNode::Leaf { ref mut parent, .. } => *parent = p,
        }
    }

    // TODO: yes i know i'm trying to write this code in a non-rusty way and i need to stop that
    pub fn key_value(&self) -> KvId {
        match self {
            RawMerkleNode::Leaf { key_value, .. } => *key_value,
            _ => panic!(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct NodeMetadata {
    pub node_type: NodeType,
    pub dirty: bool,
}

impl NodeMetadata {
    pub fn from_bytes(blob: [u8; METADATA_SIZE]) -> Result<Self, String> {
        // TODO: identify some useful structured serialization tooling we use
        Ok(Self {
            node_type: Self::node_type_from_bytes(blob)?,
            dirty: match blob[1] {
                0 => false,
                1 => true,
                other => return Err(format!("invalid dirty value: {other}")),
            },
        })
    }

    pub fn to_bytes(&self) -> [u8; METADATA_SIZE] {
        [self.node_type.to_u8(), u8::from(self.dirty)]
    }

    pub fn node_type_from_bytes(blob: [u8; METADATA_SIZE]) -> Result<NodeType, String> {
        NodeType::from_u8(blob[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_traits::Streamable;
    use hex_literal::hex;
    use std::fs;
    use std::io;
    use std::io::Write;

    const EXAMPLE_BLOB: [u8; 138] = hex!("0001ffffffff00000001000000020c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000000405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000001415161718191a1b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b");
    const HASH: Hash = [
        12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
        35, 36, 37, 38, 39, 40, 41, 42, 43,
    ];

    const EXAMPLE_ROOT: RawMerkleNode = RawMerkleNode::Internal {
        parent: NULL_PARENT,
        left: 1,
        right: 2,
        hash: HASH,
        index: 0,
    };
    const EXAMPLE_ROOT_METADATA: NodeMetadata = NodeMetadata {
        node_type: NodeType::Internal,
        dirty: true,
    };
    const EXAMPLE_LEFT_LEAF: RawMerkleNode = RawMerkleNode::Leaf {
        parent: 0,
        key_value: 0x0405_0607_0809_0A0B,
        hash: HASH,
        index: 1,
    };
    const EXAMPLE_LEFT_LEAF_METADATA: NodeMetadata = NodeMetadata {
        node_type: NodeType::Leaf,
        dirty: false,
    };
    const EXAMPLE_RIGHT_LEAF: RawMerkleNode = RawMerkleNode::Leaf {
        parent: 0,
        key_value: 0x1415_1617_1819_1A1B,
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
    fn test_node_metadata_from_to() {
        let bytes: [u8; 2] = [0, 1];
        let object = NodeMetadata::from_bytes(bytes).unwrap();
        assert_eq!(
            object,
            NodeMetadata {
                node_type: NodeType::Internal,
                dirty: true
            },
        );
        assert_eq!(object.to_bytes(), bytes);
    }

    #[test]
    fn test_load_a_python_dump() {
        // let kv_id = 0x1415161718191A1B;
        let merkle_blob = example_merkle_blob();
        merkle_blob.get_raw_node(0).unwrap();
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
        assert_eq!(last_node.parent(), NULL_PARENT);
    }

    #[test]
    fn test_get_random_leaf_node() {
        let merkle_blob = example_merkle_blob();
        let leaf = merkle_blob.get_random_leaf_node(vec![0; 8]).unwrap();
        assert_eq!(
            match leaf {
                RawMerkleNode::Internal { index, .. } | RawMerkleNode::Leaf { index, .. } => index,
            },
            2,
        );
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

        assert_eq!(merkle_blob.get_raw_node(0).unwrap(), EXAMPLE_ROOT);
        assert_eq!(merkle_blob.get_raw_node(1).unwrap(), EXAMPLE_LEFT_LEAF);
        assert_eq!(merkle_blob.get_raw_node(2).unwrap(), EXAMPLE_RIGHT_LEAF);
    }

    #[test]
    fn test_build_merkle() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        merkle_blob
            .insert(EXAMPLE_LEFT_LEAF.key_value(), EXAMPLE_LEFT_LEAF.hash())
            .unwrap();
        merkle_blob
            .insert(EXAMPLE_RIGHT_LEAF.key_value(), EXAMPLE_RIGHT_LEAF.hash())
            .unwrap();

        // TODO: just hacking here to compare with the ~wrong~ simplified reference
        let mut root = ParsedBlock::from_bytes(merkle_blob.get_block(0).unwrap(), 0).unwrap();
        root.metadata.dirty = true;
        match root.node {
            RawMerkleNode::Internal { ref mut hash, .. } => {
                *hash = HASH;
            }
            RawMerkleNode::Leaf { .. } => panic!(),
        }
        merkle_blob.blob[..BLOCK_SIZE].copy_from_slice(&root.to_bytes());

        assert_eq!(merkle_blob.blob, Vec::from(EXAMPLE_BLOB));
    }

    #[test]
    fn test_just_insert_a_bunch() {
        let mut merkle_blob = MerkleBlob::new(vec![]).unwrap();

        use std::time::{Duration, Instant};
        let mut total_time = Duration::new(0, 0);

        for i in 0..100000 {
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

        println!("total time: {total_time:?}")
        // TODO: check, well...  something
    }
}
