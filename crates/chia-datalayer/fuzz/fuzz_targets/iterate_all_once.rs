#![no_main]

use libfuzzer_sys::{
    arbitrary::{Arbitrary, Unstructured},
    fuzz_target,
};
use std::collections::HashMap;

use chia_datalayer::{
    Block, BreadthFirstIterator, Error, Hash, InsertLocation, KeyId, LeftChildFirstIterator,
    MerkleBlob, NodeType, ParentFirstIterator, TreeIndex, ValueId,
};

fuzz_target!(|data: &[u8]| {
    let mut blob = MerkleBlob::new(Vec::new()).unwrap();
    blob.check_integrity_on_drop = false;

    let mut leaf_count: usize = 0;

    let mut unstructured = Unstructured::new(data);
    while !unstructured.is_empty() {
        let key = KeyId::arbitrary(&mut unstructured).unwrap();
        let value = ValueId::arbitrary(&mut unstructured).unwrap();
        let hash = Hash::arbitrary(&mut unstructured).unwrap();

        match blob.insert(key, value, &hash, InsertLocation::Auto {}) {
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

    blob.check_integrity().unwrap();

    let raw_blob = blob.read_blob();

    let nodes_a = LeftChildFirstIterator::new(raw_blob, None)
        .collect::<Result<HashMap<TreeIndex, Block>, Error>>()
        .unwrap();
    let nodes_b = ParentFirstIterator::new(raw_blob, None)
        .collect::<Result<HashMap<TreeIndex, Block>, Error>>()
        .unwrap();
    let nodes_c = BreadthFirstIterator::new(raw_blob, None)
        .collect::<Result<HashMap<TreeIndex, Block>, Error>>()
        .unwrap();

    assert_eq!(nodes_c.len(), leaf_count);

    assert_eq!(nodes_a, nodes_b);
    let nodes_a_leafs: HashMap<TreeIndex, Block> = nodes_a
        .iter()
        .filter(|(_index, block)| block.metadata.node_type == NodeType::Leaf)
        .map(|(&k, &v)| (k, v))
        .collect();
    assert_eq!(nodes_a_leafs, nodes_c);
});
