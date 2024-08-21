use std::collections::HashMap;

type Index = usize;
type Key = Vec<u8>;

pub enum NodeType {
    Internal,
    Leaf,
}

#[derive(Debug)]
pub struct MerkleBlob {
    // TODO: shouldn't really all be pub
    pub blob: Vec<u8>,
    pub kv_to_index: HashMap<Key, Index>,
    pub free_indexes: Vec<Index>,
    pub last_allocated_index: Index,
}

pub struct NodeMetadata {
    pub node_type: NodeType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_well_something() {
        let _index: Index = 0;
        let _key = Key::new();
        let _node_type = NodeType::Internal;
        let merkle_blob = MerkleBlob {
            blob: Vec::new(),
            kv_to_index: HashMap::new(),
            free_indexes: Vec::new(),
            last_allocated_index: 0,
        };

        assert_eq!(merkle_blob.blob, Vec::new());
    }
}
