#![no_main]
use clvm_traits::{FromClvm, ToClvm};
use libfuzzer_sys::{arbitrary, fuzz_target};

use chia_fuzzing::make_tree;
use clvm_utils::CurriedProgram;
use clvmr::allocator::{Allocator, NodePtr};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let mut unstructured = arbitrary::Unstructured::new(data);
    let (input, _) = make_tree(&mut a, &mut unstructured);
    if let Ok(curry) = CurriedProgram::<NodePtr, NodePtr>::from_clvm(&a, input) {
        curry.to_clvm(&mut a).unwrap();
    }
});
