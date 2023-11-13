#![no_main]
use libfuzzer_sys::fuzz_target;

use clvm_traits::{FromPtr, ToPtr};
use clvm_utils::CurriedProgram;
use clvmr::allocator::{Allocator, NodePtr};
use fuzzing_utils::{make_tree, BitCursor};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), true);
    if let Ok(curry) = <CurriedProgram<NodePtr, NodePtr>>::from_ptr(&a, input) {
        curry.to_ptr(&mut a).unwrap();
    }
});
