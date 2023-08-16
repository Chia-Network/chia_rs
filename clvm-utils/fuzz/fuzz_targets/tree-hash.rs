#![no_main]
use libfuzzer_sys::fuzz_target;

use clvm_utils::tree_hash;
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_tree, BitCursor};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);
    let _ret = tree_hash(&a, input);
});
