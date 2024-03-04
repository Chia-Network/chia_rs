#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::gen::get_puzzle_and_solution::get_puzzle_and_solution_for_coin;
use clvmr::allocator::Allocator;
use fuzzing_utils::{make_tree, BitCursor};

const HASH: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_tree(&mut a, &mut BitCursor::new(data), false);

    let _ret = get_puzzle_and_solution_for_coin(&a, input, HASH.into(), 1337, HASH.into());
});
