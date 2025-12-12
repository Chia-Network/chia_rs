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
    serde::{node_from_bytes, node_to_bytes},
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
    let result =
        get_coinspends_for_trusted_block(&TEST_CONSTANTS, gen_prog, &vec![&[]], 0).expect("unwrap");

    for (spend, res) in spends.iter().zip(result) {
        assert_eq!(res.coin.parent_coin_info, spend.coin.parent_coin_info);
        // puzzle hash is calculated from puzzle reveal
        // so skip that as fuzz generates reveals that don't allign with Coin
        assert_eq!(res.coin.amount, spend.coin.amount);

        // convert Fuzz into minimised form for comparison
        // sometimes fuzz gibberish fails to convert to Node, if so don't compare
        if let Ok(node) = node_from_bytes(&mut a, spend.puzzle_reveal.as_ref()) {
            let minimised_bytes = node_to_bytes(&a, node).expect("node_to_bytes");
            let prog = Program::new(minimised_bytes.into());
            assert_eq!(res.puzzle_reveal, prog);
        } else {
            return;
        }
        // repeat for solution
        if let Ok(node) = node_from_bytes(&mut a, spend.solution.as_ref()) {
            let minimised_bytes = node_to_bytes(&a, node).expect("node_to_bytes");
            let prog = Program::new(minimised_bytes.into());
            assert_eq!(res.solution, prog);
        } else {
            return;
        }
    }
});
