#![no_main]
use libfuzzer_sys::fuzz_target;

use clvmr::allocator::Allocator;
use chia::fuzzing_utils::{BitCursor, make_tree};
use chia::gen::get_puzzle_and_solution::parse_coin_spend;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data));

    let _ret = parse_coin_spend(&a, input);
});
