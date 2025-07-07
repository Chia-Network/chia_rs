#![no_main]
use chia_bls::Signature;
use chia_consensus::{
    build_compressed_block::BlockBuilder, consensus_constants::TEST_CONSTANTS,
    run_block_generator::get_coinspends_with_conditions_for_trusted_block,
};
use chia_protocol::{Bytes, Coin, CoinSpend, Program, SpendBundle};
use chia_traits::Streamable;
use clvm_traits::ToClvm;
use clvmr::{
    serde::node_to_bytes,
    Allocator, NodePtr,
};
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;
use std::io::Read;

fuzz_target!(|data: &[u8]| {
    let mut spends = Vec::<CoinSpend>::new();
    let mut data = Cursor::new(data);
    let mut a = Allocator::new();
    let mut blockbuilder = BlockBuilder::new().expect("default");

    let Ok(num_of_conds) = u32::parse::<false>(&mut data) else {
        return;
    };
    let num_of_conds = num_of_conds % 100;

    let Ok(num_of_coins) = u32::parse::<false>(&mut data) else {
        return;
    };
    let num_of_coins = num_of_coins % 50;

    let bytes: &[u8] = &[1_u8];

    // a puzzle of `1` will return the solution exactly
    // so we can make the solution a list of conditions
    let one_puz = Program::new(Bytes::from(bytes));
    let Ok(one_puzhash) = clvm_utils::tree_hash_from_bytes(&one_puz) else {
        return;
    };
    let mut coinspend_conditions = Vec::<(CoinSpend, Vec<(u8, Vec<NodePtr>)>)>::new();

    for _ in 0..num_of_coins {
        let mut parent_info = [0u8; 32];
        let Ok(()) = data.read_exact(&mut parent_info) else {
            return;
        };
        let Ok(coin_amount) = u64::parse::<false>(&mut data) else {
            return;
        };
        let coin = Coin {
            parent_coin_info: parent_info.into(),
            puzzle_hash: one_puzhash.into(),
            amount: coin_amount,
        };
        let mut conds = Vec::<Vec<NodePtr>>::new();
        let mut conds_for_later_comparison = Vec::<(u8, Vec<NodePtr>)>::new();

        for _ in 0..num_of_conds {
            let mut cond_vec = Vec::<NodePtr>::new();
            let mut buf = [0u8; 1];
            let Ok(()) = data.read_exact(&mut buf) else {
                return;
            };
            let opcode: u8 = buf[0] % 100;
            cond_vec.push(opcode.to_clvm(&mut a).expect("opcode"));
            let mut arg_one = [0u8; 32];
            let mut arg_two = [0u8; 32];
            let Ok(()) = data.read_exact(&mut arg_one) else {
                return;
            };
            let Ok(()) = data.read_exact(&mut arg_two) else {
                return;
            };
            cond_vec.push(arg_one.to_clvm(&mut a).expect("arg one"));
            conds.push(cond_vec.clone());
            conds_for_later_comparison.push((opcode, cond_vec[1..].to_vec()));
        }
        let solution = conds.to_clvm(&mut a).expect("vec of nodes");
        let solution_bytes = node_to_bytes(&a, solution).expect("node to bytes");

        let coinspend = CoinSpend {
            coin,
            puzzle_reveal: one_puz.clone(),
            solution: Program::new(solution_bytes.into()),
        };
        spends.push(coinspend.clone());
        coinspend_conditions.push((coinspend, conds_for_later_comparison));
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
        get_coinspends_with_conditions_for_trusted_block(&TEST_CONSTANTS, gen_prog, vec![&[]], 0)
            .expect("unwrap");

    for ((original_cs, original_conds), (res_cs, res_conds)) in
        coinspend_conditions.iter().zip(result)
    {
        assert_eq!(
            original_cs.coin.parent_coin_info,
            res_cs.coin.parent_coin_info
        );
        // puzzle hash is calculated from puzzle reveal
        // so skip that as fuzz generates reveals that don't allign with Coin
        assert_eq!(*original_cs, res_cs);

        for (orig_cond, res_cond) in original_conds.iter().zip(res_conds) {
            assert_eq!(orig_cond.0 as u32, res_cond.0);
            for (orig_arg, res_cond) in orig_cond.1.clone().iter().zip(res_cond.1) {
                let bytes = node_to_bytes(&a, *orig_arg).expect("arg nodetobytes");
                assert_eq!(bytes, res_cond);
            }
        }
    }
});
