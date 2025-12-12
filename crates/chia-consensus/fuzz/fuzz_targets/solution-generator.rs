#![no_main]
use chia_consensus::solution_generator::{calculate_generator_length, solution_generator};
use chia_protocol::{Coin, CoinSpend};
use chia_traits::Streamable;
use clvmr::{
    Allocator,
    serde::{node_from_bytes_backrefs, node_to_bytes},
};
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut spends = Vec::<CoinSpend>::new();
    let mut generator_input = Vec::<(Coin, Vec<u8>, Vec<u8>)>::new();
    let mut data = Cursor::new(data);
    let mut discrepancy: i64 = 0;
    let mut a = Allocator::new();
    while let Ok(spend) = CoinSpend::parse::<false>(&mut data) {
        spends.push(spend.clone());
        generator_input.push((
            spend.coin,
            spend.puzzle_reveal.to_vec(),
            spend.solution.to_vec(),
        ));
        // Check for atoms which can be represented in a smaller form
        let node = node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref()).expect("node");
        let puz = node_to_bytes(&a, node).expect("bytes");
        discrepancy += spend.puzzle_reveal.as_ref().len() as i64 - puz.len() as i64;
        let node = node_from_bytes_backrefs(&mut a, spend.solution.as_ref()).expect("node");
        let sol = node_to_bytes(&a, node).expect("bytes");
        discrepancy += spend.solution.as_ref().len() as i64 - sol.len() as i64;
    }
    if spends.is_empty() {
        return;
    }
    let result = solution_generator(generator_input).expect("solution_generator");

    assert_eq!(
        result.len() as i64,
        calculate_generator_length(spends) as i64 - discrepancy
    );
});
