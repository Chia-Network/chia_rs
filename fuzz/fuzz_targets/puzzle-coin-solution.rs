#![no_main]
use libfuzzer_sys::fuzz_target;

use clvmr::allocator::Allocator;
use chia::fuzzing_utils::{BitCursor, make_tree};
use chia::gen::get_puzzle_and_solution::get_puzzle_and_solution_for_coin;

const HASH: [u8;32] = [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data));

    let _ret = get_puzzle_and_solution_for_coin(&a, input, HASH.into(), 1337, HASH.into());
});
