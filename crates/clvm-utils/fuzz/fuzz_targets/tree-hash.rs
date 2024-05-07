#![no_main]
use libfuzzer_sys::fuzz_target;

use clvm_utils::{tree_hash, tree_hash_cached, TreeHash};
use clvmr::{Allocator, NodePtr};
use fuzzing_utils::{make_tree, BitCursor};
use std::collections::{HashMap, HashSet};

use clvmr::serde::{node_from_bytes_backrefs_record, node_to_bytes_backrefs};

fn test_hash(a: &Allocator, node: NodePtr, backrefs: &HashSet<NodePtr>) {
    let hash1 = tree_hash(a, node);

    let mut cache = HashMap::<NodePtr, TreeHash>::new();
    let hash2 = tree_hash_cached(a, node, backrefs, &mut cache);
    assert_eq!(hash1, hash2);
}

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);
    test_hash(&a, input, &HashSet::new());

    let bytes = node_to_bytes_backrefs(&a, input).expect("node_to_bytes_backrefs");
    let (input, backrefs) =
        node_from_bytes_backrefs_record(&mut a, &bytes).expect("node_from_bytes_backrefs_record");
    test_hash(&a, input, &backrefs);
});
