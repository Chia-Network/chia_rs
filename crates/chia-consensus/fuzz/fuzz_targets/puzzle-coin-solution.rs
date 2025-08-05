#![no_main]
use libfuzzer_sys::{arbitrary, fuzz_target};

use chia_consensus::get_puzzle_and_solution::get_puzzle_and_solution_for_coin;
use chia_protocol::Coin;
use clvm_fuzzing::make_tree;
use clvmr::allocator::Allocator;

const HASH: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let mut unstructured = arbitrary::Unstructured::new(data);
    let (input, _) = make_tree(&mut a, &mut unstructured);

    let _ret =
        get_puzzle_and_solution_for_coin(&a, input, &Coin::new(HASH.into(), HASH.into(), 1337));
});
