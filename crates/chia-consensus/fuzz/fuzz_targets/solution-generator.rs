#![no_main]
use chia_protocol::{Coin, CoinSpend};
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;
use chia_consensus::gen::solution_generator::{calculate_generator_length, solution_generator};

fuzz_target!(|data: &[u8]| {
    let mut spends = Vec::<CoinSpend>::new();
    let mut generator_input = Vec::<(Coin, Vec<u8>, Vec<u8>)>::new();
    for _i in 1..1000 {
        let Ok(spend) = CoinSpend::parse::<false>(&mut Cursor::new(data)) else {
            return;
        };
        spends.push(spend.clone());
        generator_input.push((spend.coin, spend.puzzle_reveal.to_vec(), spend.solution.to_vec()));
    }

    let result = solution_generator(generator_input).expect("solution_generator");

    assert_eq!(result.len(), calculate_generator_length(spends));

});