#![no_main]
use clvm_traits::{FromClvm, ToClvm};
use libfuzzer_sys::fuzz_target;

use clvm_fuzzing::ArbitraryClvmTree;
use clvm_utils::CurriedProgram;
use clvmr::allocator::NodePtr;

fuzz_target!(|input: ArbitraryClvmTree| {
    let mut a = input.allocator;
    if let Ok(curry) = CurriedProgram::<NodePtr, NodePtr>::from_clvm(&a, input.tree) {
        curry.to_clvm(&mut a).unwrap();
    }
});
