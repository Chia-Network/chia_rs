#![no_main]
use libfuzzer_sys::fuzz_target;

use chia::gen::get_puzzle_and_solution::parse_coin_spend;
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_tree, BitCursor};

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);

    let _ret = parse_coin_spend(&a, input);
});
