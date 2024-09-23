#![no_main]
use chia_consensus::gen::solution_generator::{calculate_generator_length, solution_generator};
use chia_protocol::{Coin, CoinSpend};
use chia_traits::Streamable;
use clvmr::{
    serde::{node_from_bytes_backrefs, node_to_bytes},
    Allocator,
};
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut spends = Vec::<CoinSpend>::new();
    let mut generator_input = Vec::<(Coin, Vec<u8>, Vec<u8>)>::new();
    let mut data = Cursor::new(data);
    let mut discrepancy: usize = 0;
    let mut a = Allocator::new();
    while let Ok(spend) = CoinSpend::parse::<false>(&mut data) {
        spends.push(spend.clone());
        generator_input.push((
            spend.coin,
            spend.puzzle_reveal.to_vec(),
            spend.solution.to_vec(),
        ));
        // Check if puzzle or solution are atoms which can represented in a smaller form
        let node = node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref()).expect("atom");
        if node.is_atom() {
            let puz = node_to_bytes(&a, node).expect("bytes");
            discrepancy += spend.puzzle_reveal.as_ref().len() - puz.len();
        }
        let node = node_from_bytes_backrefs(&mut a, spend.solution.as_ref()).expect("atom");
        if node.is_atom() {
            let sol = node_to_bytes(&a, node).expect("bytes");
            discrepancy += spend.solution.as_ref().len() - sol.len();
        }
    }
    if spends.is_empty() {
        return;
    }
    let Ok(result) = solution_generator(generator_input) else {
        return;
    };

    if result.len() != calculate_generator_length(spends.clone()) - discrepancy {
        panic!("Debug, spends are: {:?}", spends);
    }

    assert_eq!(
        result.len(),
        calculate_generator_length(spends) - discrepancy
    );
});
