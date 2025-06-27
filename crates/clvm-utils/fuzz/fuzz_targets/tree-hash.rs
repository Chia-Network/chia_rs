#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_fuzz::{make_tree, BitCursor};
use clvm_utils::{tree_hash, tree_hash_cached, TreeCache};
use clvmr::{Allocator, NodePtr};

use clvmr::serde::{node_from_bytes_backrefs, node_to_bytes_backrefs};

fn test_hash(a: &Allocator, node: NodePtr) {
    let hash1 = tree_hash(a, node);

    let mut cache = TreeCache::default();
    let hash2 = tree_hash_cached(a, node, &mut cache);
    assert_eq!(hash1, hash2);
}

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);
    test_hash(&a, input);

    let bytes = node_to_bytes_backrefs(&a, input).expect("node_to_bytes_backrefs");
    let input = node_from_bytes_backrefs(&mut a, &bytes).expect("node_from_bytes_backrefs");
    test_hash(&a, input);
});
