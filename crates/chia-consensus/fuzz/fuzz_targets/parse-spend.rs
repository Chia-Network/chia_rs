#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::general::get_puzzle_and_solution::parse_coin_spend;
use chia_fuzz::{make_list, BitCursor};
use clvmr::allocator::Allocator;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let input = make_list(&mut a, &mut BitCursor::new(data));

    let _ret = parse_coin_spend(&a, input);
});
