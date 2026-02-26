#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::get_puzzle_and_solution::get_puzzle_and_solution_for_coin;
use chia_protocol::Coin;
use clvm_fuzzing::ArbitraryClvmTree;

const HASH: [u8; 32] = [0_u8; 32];

fuzz_target!(|input: ArbitraryClvmTree| {
    let _ret = get_puzzle_and_solution_for_coin(
        &input.allocator,
        input.tree,
        &Coin::new(HASH.into(), HASH.into(), 1337),
    );
});
