#![no_main]
use chia_bls::Signature;
use chia_consensus::{
    build_compressed_block::BlockBuilder, consensus_constants::TEST_CONSTANTS,
    run_block_generator::get_coinspends_with_conditions_for_trusted_block,
};
use chia_protocol::{Bytes, Coin, CoinSpend, Program, SpendBundle};
use chia_traits::Streamable;
use clvm_traits::ToClvm;
use clvmr::{serde::node_to_bytes, Allocator, NodePtr};
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
    let num_of_conds = (num_of_conds % 100) + 1;

    let Ok(num_of_coins) = u32::parse::<false>(&mut data) else {
        return;
    };
    let num_of_coins = (num_of_coins % 50) + 1;

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
            let opcode: u8 = (buf[0] % 100) + 1;
            cond_vec.push(opcode.to_clvm(&mut a).expect("opcode"));
            let mut arg_one = [0u8; 32];
            let mut arg_two = [0u8; 32];
            let Ok(()) = data.read_exact(&mut arg_one) else {
                return;
            };
            let Ok(()) = data.read_exact(&mut arg_two) else {
                return;
            };
            cond_vec.push(a.new_atom(&arg_one).expect("arg one"));
            cond_vec.push(a.new_atom(&arg_two).expect("arg one"));
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
    println!("DEBUG ORIGINAL: {coinspend_conditions:?}");
    println!("DEBUG RESPONSE: {result:?}");
    for (original_cs, original_conds) in &coinspend_conditions {
        let found = result.iter().any(|(res_cs, res_conds)| {
            // if coinspends aren't the same
            if original_cs != res_cs {
                return false;
            }

            if original_conds.len() != res_conds.len() {
                return false;
            }

            for orig_cond in original_conds {
                let matching_cond = res_conds.iter().find(|(opcode, args)| {
                    if orig_cond.0 as u32 != *opcode {
                        println!("DEBUG ERROR: {orig_cond:?} != {opcode:?}");
                        return false;
                    }

                    if orig_cond.1.len() != args.len() {
                        println!("DEBUG ERROR: wrong length");
                        return false;
                    }

                    // compare args now we've found a match for original condition
                    for (orig_arg, res_arg) in orig_cond.1.iter().zip(args) {
                        let bytes = node_to_bytes(&a, *orig_arg).expect("arg nodetobytes");
                        if &bytes != res_arg {
                            println!("DEBUG ERROR: {bytes:?} != {res_arg:?}");
                            return false;
                        }
                    }

                    true
                });

                if matching_cond.is_none() {
                    return false;
                }
            }

            true
        });

        assert!(
            found,
            "Original CoinSpend and Conditions pair not found in result"
        );
    }
});
