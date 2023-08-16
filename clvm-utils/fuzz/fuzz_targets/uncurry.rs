#![no_main]
use libfuzzer_sys::fuzz_target;

use clvm_utils::uncurry;
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_tree, BitCursor};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), true);
    let _ret = uncurry(&a, input);
});
