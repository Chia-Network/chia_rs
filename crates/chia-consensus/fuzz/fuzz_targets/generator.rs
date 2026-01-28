#![no_main]
use chia_bls::Signature;
use chia_consensus::{
    build_compressed_block::BlockBuilder, consensus_constants::TEST_CONSTANTS,
    run_block_generator::get_coinspends_for_trusted_block,
};
use chia_protocol::{CoinSpend, Program, SpendBundle};
use chia_traits::Streamable;
use clvmr::{
    Allocator,
    serde::{node_from_bytes_backrefs, node_to_bytes},
};
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut spends = Vec::<CoinSpend>::new();
    let mut data = Cursor::new(data);
    let mut a = Allocator::new();
    let mut blockbuilder = BlockBuilder::new().expect("default");

    while let Ok(spend) = CoinSpend::parse::<false>(&mut data) {
        spends.push(spend.clone());
    }
    if spends.is_empty() {
        return;
    }
    let spend_bundle = SpendBundle {
        coin_spends: spends.clone(),
        aggregated_signature: Signature::default(),
    };
    blockbuilder
        .add_spend_bundles([spend_bundle], 0, &TEST_CONSTANTS)
        .expect("add spend");
    let Ok((generator, _sig, _cost)) = blockbuilder.finalize(&TEST_CONSTANTS) else {
        return;
    };
    let gen_prog = &Program::new(generator.clone().into());
    let mut result = get_coinspends_for_trusted_block(&TEST_CONSTANTS, gen_prog, &vec![&[]], 0)
        .expect("get_coinspends_for_trusted_block");

    assert_eq!(spends.len(), result.len());

    // spends are serialized in reverse order, since lisp lists are built from
    // end to beginning.
    result.reverse();

    for (spend, res) in spends.iter().zip(result) {
        assert_eq!(res.coin.parent_coin_info, spend.coin.parent_coin_info);
        // puzzle hash is calculated from puzzle reveal
        // so skip that as fuzz generates reveals that don't allign with Coin
        assert_eq!(res.coin.amount, spend.coin.amount);

        // if the serialization would have been > 2MB.
        // get_coinspends_for_trusted block skipped this puzzle
        if res.puzzle_reveal != Program::default() {
            let node = node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref())
                .expect("invalid puzzle reveal");
            let minimised_bytes = node_to_bytes(&a, node).expect("node_to_bytes");
            let prog = Program::new(minimised_bytes.into());
            assert_eq!(res.puzzle_reveal, prog);
        }
        // repeat for solution

        // if the serialization would have been > 2MB.
        // get_coinspends_for_trusted block skipped this solution
        if res.solution != Program::default() {
            let node = node_from_bytes_backrefs(&mut a, spend.solution.as_ref())
                .expect("invalid solution");
            let minimised_bytes = node_to_bytes(&a, node).expect("node_to_bytes");
            let prog = Program::new(minimised_bytes.into());
            assert_eq!(res.solution, prog);
        }
    }
});
