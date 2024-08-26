// use std::collections::HashMap;

type TreeIndex = u32;
// type Key = Vec<u8>;
type Hash = [u8; 32];
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
            other => panic!("unknown NodeType value: {}", other),
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

#[derive(Debug)]
pub struct MerkleBlob {
    blob: Vec<u8>,
    free_indexes: Vec<TreeIndex>,
}

// TODO: clearly shouldn't be hard coded
const METADATA_SIZE: usize = 2;
// TODO: clearly shouldn't be hard coded
const DATA_SIZE: usize = 44;
const SPACING: usize = METADATA_SIZE + DATA_SIZE;

// TODO: probably bogus and overflowing or somesuch
const NULL_PARENT: TreeIndex = 0xffffffff; // 1 << (4 * 8) - 1;

impl MerkleBlob {
    pub fn insert(&mut self) -> Result<(), String> {
        // TODO: garbage just to use stuff
        let index = self.get_new_index();
        self.insert_entry_to_blob(index, [0; SPACING])?;

        Ok(())
    }

    fn get_new_index(&mut self) -> TreeIndex {
        match self.free_indexes.pop() {
            None => (self.blob.len() / SPACING) as TreeIndex,
            Some(new_index) => new_index,
        }
    }

    fn insert_entry_to_blob(
        &mut self,
        index: TreeIndex,
        entry: [u8; SPACING],
    ) -> Result<(), String> {
        let extend_index = (self.blob.len() / SPACING) as TreeIndex;
        if index > extend_index {
            return Err(format!("index out of range: {index}"));
        } else if index == extend_index {
            self.blob.extend_from_slice(&entry);
        } else {
            let start = index as usize * SPACING;
            self.blob[start..start + SPACING].copy_from_slice(&entry);
        }

        Ok(())
    }

    pub fn get_raw_node(&self, index: TreeIndex) -> Result<RawMerkleNode, String> {
        // TODO: handle invalid indexes?
        // TODO: handle overflows?
        let metadata_start = index as usize * SPACING;
        let data_start = metadata_start + METADATA_SIZE;
        let end = data_start + DATA_SIZE;

        let metadata_blob: [u8; METADATA_SIZE] = self
            .blob
            .get(metadata_start..data_start)
            .ok_or(format!(
                "metadata blob out of bounds: {} {} {}",
                self.blob.len(),
                metadata_start,
                data_start
            ))?
            .try_into()
            .map_err(|e| format!("metadata blob wrong size: {e}"))?;
        let data_blob: [u8; DATA_SIZE] = self
            .blob
            .get(data_start..end)
            .ok_or("data blob out of bounds".to_string())?
            .try_into()
            .map_err(|e| format!("data blob wrong size: {e}"))?;
        let metadata = match NodeMetadata::from_bytes(metadata_blob) {
            Ok(metadata) => metadata,
            Err(message) => return Err(format!("failed loading metadata: {message})")),
        };
        Ok(
            match RawMerkleNode::from_bytes(metadata, index, data_blob) {
                Ok(node) => node,
                Err(message) => return Err(format!("failed loading raw node: {message}")),
            },
        )
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
}

#[derive(Debug)]
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
        metadata: NodeMetadata,
        index: TreeIndex,
        blob: [u8; DATA_SIZE],
    ) -> Result<Self, String> {
        // TODO: add Err results
        match metadata.node_type {
            NodeType::Internal => Ok(RawMerkleNode::Internal {
                // TODO: get these right
                parent: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[0..4]).unwrap()),
                left: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[4..8]).unwrap()),
                right: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[8..12]).unwrap()),
                hash: <[u8; 32]>::try_from(&blob[12..44]).unwrap(),
                index,
            }),
            NodeType::Leaf => Ok(RawMerkleNode::Leaf {
                // TODO: this try from really right?
                parent: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[0..4]).unwrap()),
                key_value: KvId::from_be_bytes(<[u8; 8]>::try_from(&blob[4..12]).unwrap()),
                hash: Hash::try_from(&blob[12..44]).unwrap(),
                index,
            }),
        }
    }

    pub fn parent(&self) -> TreeIndex {
        match self {
            RawMerkleNode::Internal { parent, .. } => *parent,
            RawMerkleNode::Leaf { parent, .. } => *parent,
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
            node_type: NodeType::from_u8(blob[0])?,
            dirty: match blob[1] {
                0 => false,
                1 => true,
                other => return Err(format!("invalid dirty value: {other}")),
            },
        })
    }

    pub fn to_bytes(&self) -> [u8; METADATA_SIZE] {
        [
            self.node_type.to_u8(),
            match self.dirty {
                false => 0,
                true => 1,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::*;

    fn example_blob() -> MerkleBlob {
        let something = hex!("0001ffffffff00000001000000020c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000000405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000001415161718191a1b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b");
        MerkleBlob {
            blob: Vec::from(something),
            free_indexes: vec![],
        }
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
        let merkle_blob = example_blob();
        merkle_blob.get_raw_node(0).unwrap();
    }

    #[test]
    fn test_get_lineage() {
        let merkle_blob = example_blob();
        let lineage = merkle_blob.get_lineage(2).unwrap();
        for node in &lineage {
            println!("{node:?}");
        }
        assert_eq!(lineage.len(), 2);
        let last_node = lineage.last().unwrap();
        assert_eq!(last_node.parent(), NULL_PARENT);
    }
}
