#![no_main]
use chia_consensus::gen::solution_generator::{calculate_generator_length, solution_generator};
use chia_protocol::{Coin, CoinSpend};
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;
use core::panic;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut spends = Vec::<CoinSpend>::new();
    let mut generator_input = Vec::<(Coin, Vec<u8>, Vec<u8>)>::new();
    let mut data = Cursor::new(data);
    while let Ok(spend) = CoinSpend::parse::<true>(&mut data) {
        spends.push(spend.clone());
        generator_input.push((
            spend.coin,
            spend.puzzle_reveal.to_vec(),
            spend.solution.to_vec(),
        ));
    }
    if spends.is_empty() {
        return;
    }
    let Ok(result) = solution_generator(generator_input) else {
        return;
    };

    if result.len() !=  calculate_generator_length(spends.clone()){
        panic!("DEBUG: {:?}", spends);
    }
    assert_eq!(result.len(), calculate_generator_length(spends));
});
