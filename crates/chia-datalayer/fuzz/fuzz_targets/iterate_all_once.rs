#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

use chia_datalayer::{
    Block, BreadthFirstIterator, Error, Hash, InsertLocation, KeyId, LeftChildFirstIterator,
    MerkleBlob, NodeType, ParentFirstIterator, TreeIndex, ValueId,
};

fuzz_target!(|args: Vec<(KeyId, ValueId, Hash)>| {
    let mut blob = MerkleBlob::new(Vec::new()).expect("construct MerkleBlob");
    blob.check_integrity_on_drop = false;

    let mut leaf_count: usize = 0;

    for (key, value, hash) in &args {
        match blob.insert(*key, *value, hash, InsertLocation::Auto {}) {
            Ok(_) => {
                leaf_count += 1;
            }
            // should remain valid through these errors
            Err(Error::KeyAlreadyPresent()) => continue,
            Err(Error::HashAlreadyPresent()) => continue,
            // other errors should not be occurring
            Err(error) => panic!("unexpected error while inserting: {:?}", error),
        };
    }

    blob.check_integrity().expect("check integrity");

    let raw_blob = blob.read_blob();

    let nodes_a = LeftChildFirstIterator::new(raw_blob, None)
        .collect::<Result<HashMap<TreeIndex, Block>, Error>>()
        .expect("left child first iterator");
    let nodes_b = ParentFirstIterator::new(raw_blob, None)
        .collect::<Result<HashMap<TreeIndex, Block>, Error>>()
        .expect("parent first iterator");
    let nodes_c = BreadthFirstIterator::new(raw_blob, None)
        .collect::<Result<HashMap<TreeIndex, Block>, Error>>()
        .expect("breadth first iterator");

    assert_eq!(nodes_c.len(), leaf_count);

    assert_eq!(nodes_a, nodes_b);
    let nodes_a_leafs: HashMap<TreeIndex, Block> = nodes_a
        .iter()
        .filter(|(_index, block)| block.metadata.node_type == NodeType::Leaf)
        .map(|(&k, &v)| (k, v))
        .collect();
    assert_eq!(nodes_a_leafs, nodes_c);
});
