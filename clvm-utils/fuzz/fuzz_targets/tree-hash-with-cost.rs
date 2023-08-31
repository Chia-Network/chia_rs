#![no_main]
#[cfg(fuzzing)]
use clvm_utils::cmp_hash;
use clvmr::Allocator;
use fuzzing_utils::{make_tree, BitCursor};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);
    #[cfg(fuzzing)]
    cmp_hash(&mut a, input);
});
