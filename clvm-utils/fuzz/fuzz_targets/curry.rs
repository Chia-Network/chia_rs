#![no_main]
use libfuzzer_sys::fuzz_target;

use clvm_traits::AllocatorExt;
use clvm_utils::CurriedProgram;
use clvmr::allocator::{Allocator, NodePtr};
use fuzzing_utils::{make_tree, BitCursor};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), true);
    if let Ok(curry) = a.value_from_ptr::<CurriedProgram<NodePtr, NodePtr>>(input) {
        a.value_to_ptr(curry).unwrap();
    }
});
