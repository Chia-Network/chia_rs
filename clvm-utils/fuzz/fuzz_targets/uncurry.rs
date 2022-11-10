#![no_main]
use libfuzzer_sys::fuzz_target;

use clvmr::allocator::Allocator;
use chia::fuzzing_utils::{BitCursor, make_tree};
use clvm_utils::uncurry::uncurry;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), true);
    let _ret = uncurry(&a, input);
});
