#![no_main]
use clvm_traits::{FromClvm, ToClvm};
use libfuzzer_sys::fuzz_target;

use chia_fuzz::{make_tree, BitCursor};
use clvm_utils::CurriedProgram;
use clvmr::allocator::{Allocator, NodePtr};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), true);
    if let Ok(curry) = CurriedProgram::<NodePtr, NodePtr>::from_clvm(&a, input) {
        curry.to_clvm(&mut a).unwrap();
    }
});
