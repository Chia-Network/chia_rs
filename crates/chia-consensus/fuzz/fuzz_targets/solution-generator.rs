#![no_main]
use chia_consensus::solution_generator::{calculate_generator_length, solution_generator};
use chia_protocol::{Coin, CoinSpend};
use clvmr::{
    Allocator,
    serde::{node_from_bytes_backrefs, node_to_bytes},
};
use libfuzzer_sys::{Corpus, fuzz_target};

fuzz_target!(|spends: Vec<CoinSpend>| -> Corpus {
    let mut generator_input = Vec::<(Coin, Vec<u8>, Vec<u8>)>::new();
    let mut discrepancy: i64 = 0;

    if spends.is_empty() {
        return Corpus::Reject;
    }
    let mut a = Allocator::new();
    let checkpoint = a.checkpoint();
    for spend in &spends {
        a.restore_checkpoint(&checkpoint);
        generator_input.push((
            spend.coin,
            spend.puzzle_reveal.to_vec(),
            spend.solution.to_vec(),
        ));
        // Check for atoms which can be represented in a smaller form
        let Ok(node) = node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref()) else {
            return Corpus::Reject;
        };
        let Ok(puz) = node_to_bytes(&a, node) else {
            return Corpus::Reject;
        };
        discrepancy += spend.puzzle_reveal.as_ref().len() as i64 - puz.len() as i64;
        let Ok(node) = node_from_bytes_backrefs(&mut a, spend.solution.as_ref()) else {
            return Corpus::Reject;
        };
        let Ok(sol) = node_to_bytes(&a, node) else {
            return Corpus::Reject;
        };
        discrepancy += spend.solution.as_ref().len() as i64 - sol.len() as i64;
    }
    let result = solution_generator(generator_input).expect("solution_generator");

    assert_eq!(
        result.len() as i64,
        calculate_generator_length(spends) as i64 - discrepancy
    );
    Corpus::Keep
});
