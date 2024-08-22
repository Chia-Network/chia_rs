// use std::collections::HashMap;

type TreeIndex = u32;
// type Key = Vec<u8>;
type Hash = [u8; 32];
type KvId = Hash;

#[derive(Debug, Hash, Eq, PartialEq)]

pub enum NodeType {
    Internal,
    Leaf,
}

impl NodeType {
    pub fn load(&value: &u8) -> Self {
        // TODO: identify some useful structured serialization tooling we use
        // TODO: find a better way to tie serialization values to enumerators
        match value {
            0 => NodeType::Internal,
            1 => NodeType::Leaf,
            other => panic!("unknown NodeType value: {}", other),
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
    // TODO: shouldn't really all be pub
    pub blob: Vec<u8>,
}

// TODO: clearly shouldnt' be hard coded
const METADATA_SIZE: usize = 2;
// TODO: clearly shouldnt' be hard coded
const DATA_SIZE: usize = 68;
const SPACING: usize = METADATA_SIZE + DATA_SIZE;

impl MerkleBlob {
    pub fn get_raw_node(&self, index: TreeIndex) -> Result<RawMerkleNode, String> {
        // TODO: handle invalid indexes?
        // TODO: handle overflows?
        let metadata_start = index as usize * SPACING;
        let data_start = metadata_start + METADATA_SIZE;
        let end = data_start + DATA_SIZE;

        let metadata_blob: [u8; METADATA_SIZE] = self
            .blob
            .get(metadata_start..data_start)
            .ok_or(String::from("metadata blob out of bounds"))?
            .try_into()
            .map_err(|e| format!("metadata blob wrong size: {e}"))?;
        let data_blob: [u8; DATA_SIZE] = self
            .blob
            .get(data_start..end)
            .ok_or(String::from("data blob out of bounds"))?
            .try_into()
            .map_err(|e| format!("data blob wrong size: {e}"))?;
        let metadata = match NodeMetadata::load(metadata_blob) {
            Ok(metadata) => metadata,
            Err(message) => return Err(format!("failed loading metadata: {message})")),
        };
        Ok(match RawMerkleNode::load(metadata, 0, data_blob) {
            Ok(node) => node,
            Err(message) => return Err(format!("failed loading raw node: {message}")),
        })
    }
}

pub enum RawMerkleNode {
    Root {
        left: TreeIndex,
        right: TreeIndex,
        hash: Hash,
        // TODO: kinda feels questionable having it be aware of its own location
        // TODO: just always at zero?
        index: TreeIndex,
    },
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
    pub fn load(
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
                hash: <[u8; 32]>::try_from(&blob[12..46]).unwrap(),
                index,
            }),
            NodeType::Leaf => Ok(RawMerkleNode::Leaf {
                // TODO: this try from really right?
                parent: TreeIndex::from_be_bytes(<[u8; 4]>::try_from(&blob[0..4]).unwrap()),
                key_value: KvId::try_from(&blob[4..36]).unwrap(),
                hash: Hash::try_from(&blob[36..68]).unwrap(),
                index,
            }),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct NodeMetadata {
    pub node_type: NodeType,
    pub dirty: bool,
}

impl NodeMetadata {
    pub fn load(blob: [u8; METADATA_SIZE]) -> Result<Self, String> {
        // TODO: identify some useful structured serialization tooling we use
        Ok(Self {
            node_type: NodeType::load(&blob[0]),
            dirty: match blob[1] {
                0 => false,
                1 => true,
                other => return Err(format!("invalid dirty value: {other}")),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let a: [u8; 2] = [0, 1];
        assert_eq!(
            NodeMetadata::load(a),
            Ok(NodeMetadata {
                node_type: NodeType::Internal,
                dirty: true
            })
        );
    }
}
