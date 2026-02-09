#![no_main]
use libfuzzer_sys::fuzz_target;

use clvm_utils::{TreeCache, tree_hash, tree_hash_cached};
use clvmr::{Allocator, NodePtr};

use clvm_fuzzing::ArbitraryClvmTree;
use clvmr::serde::{node_from_bytes_backrefs, node_to_bytes_backrefs};

fn test_hash(a: &Allocator, node: NodePtr) {
    let hash1 = tree_hash(a, node);

    let mut cache = TreeCache::default();
    let hash2 = tree_hash_cached(a, node, &mut cache);
    assert_eq!(hash1, hash2);
}

fuzz_target!(|input: ArbitraryClvmTree| {
    let mut a = input.allocator;
    test_hash(&a, input.tree);

    let bytes = node_to_bytes_backrefs(&a, input.tree).expect("node_to_bytes_backrefs");
    let input = node_from_bytes_backrefs(&mut a, &bytes).expect("node_from_bytes_backrefs");
    test_hash(&a, input);
});
