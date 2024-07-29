use crate::consensus_constants::ConsensusConstants;
use crate::gen::condition_tools::make_aggsig_final_message;
use crate::gen::flags::{ALLOW_BACKREFS, DISALLOW_INFINITY_G1, ENABLE_MESSAGE_CONDITIONS};
use crate::gen::opcodes::{
    AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
};
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::validation_error::ErrorCode;
use crate::npc_result::get_conditions_from_spendbundle;
use chia_bls::BlsCache;
use chia_protocol::SpendBundle;
use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV};
use std::time::{Duration, Instant};

// currently in mempool_manager.py
// called in threads from pre_validate_spend_bundle()
// returns (error, cached_results, new_cache_entries, duration)
pub fn validate_clvm_and_signature(
    spend_bundle: &SpendBundle,
    max_cost: u64,
    constants: &ConsensusConstants,
    height: u32,
    cache: &BlsCache,
) -> Result<(OwnedSpendBundleConditions, Duration), ErrorCode> {
    let start_time = Instant::now();
    let npcresult = get_conditions_from_spendbundle(spend_bundle, max_cost, height, constants)
        .map_err(|e| e.1)?;
    let iter = npcresult.spends.iter().flat_map(|spend| {
        let condition_items_pairs = [
            (AGG_SIG_PARENT, &spend.agg_sig_parent),
            (AGG_SIG_PUZZLE, &spend.agg_sig_puzzle),
            (AGG_SIG_AMOUNT, &spend.agg_sig_amount),
            (AGG_SIG_PUZZLE_AMOUNT, &spend.agg_sig_puzzle_amount),
            (AGG_SIG_PARENT_AMOUNT, &spend.agg_sig_parent_amount),
            (AGG_SIG_PARENT_PUZZLE, &spend.agg_sig_parent_puzzle),
            (AGG_SIG_ME, &spend.agg_sig_me),
        ];
        condition_items_pairs
            .into_iter()
            .flat_map(move |(condition, items)| {
                let spend_clone = spend.clone();
                items.iter().map(move |(pk, msg)| {
                    (
                        pk,
                        make_aggsig_final_message(
                            condition,
                            msg.as_slice(),
                            &spend_clone,
                            constants,
                        ),
                    )
                })
            })
    });
    let unsafe_items = npcresult
        .agg_sig_unsafe
        .iter()
        .map(|(pk, msg)| (pk, msg.as_slice().to_vec()));
    let iter = iter.chain(unsafe_items);
    // Verify aggregated signature
    let result = cache.aggregate_verify(iter, &spend_bundle.aggregated_signature);
    if !result {
        return Err(ErrorCode::BadAggregateSignature);
    }
    Ok((npcresult, start_time.elapsed()))
}

pub fn get_flags_for_height_and_constants(height: u32, constants: &ConsensusConstants) -> u32 {
    let mut flags: u32 = 0;

    if height >= constants.soft_fork4_height {
        flags |= ENABLE_MESSAGE_CONDITIONS;
    }

    if height >= constants.soft_fork5_height {
        flags |= DISALLOW_INFINITY_G1;
    }

    if height >= constants.hard_fork_height {
        //  the hard-fork initiated with 2.0. To activate June 2024
        //  * costs are ascribed to some unknown condition codes, to allow for
        // soft-forking in new conditions with cost
        //  * a new condition, SOFTFORK, is added which takes a first parameter to
        //    specify its cost. This allows soft-forks similar to the softfork
        //    operator
        //  * BLS operators introduced in the soft-fork (behind the softfork
        //    guard) are made available outside of the guard.
        //  * division with negative numbers are allowed, and round toward
        //    negative infinity
        //  * AGG_SIG_* conditions are allowed to have unknown additional
        //    arguments
        //  * Allow the block generator to be serialized with the improved clvm
        //   serialization format (with back-references)
        flags = flags | ENABLE_BLS_OPS_OUTSIDE_GUARD | ENABLE_FIXED_DIV | ALLOW_BACKREFS;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::gen::condition_tools::u64_to_bytes;
    use chia_bls::{sign, G2Element, SecretKey, Signature};
    use chia_protocol::{Bytes, Bytes32};
    use chia_protocol::{Coin, CoinSpend, Program};
    use clvm_utils::tree_hash_atom;
    use hex::FromHex;
    use hex_literal::hex;
    use std::sync::Arc;

    #[test]
    fn test_validate_no_pks() {
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            hex!("3333333333333333333333333333333333333333333333333333333333333333").into(),
            1,
        );

        let solution = Bytes::new(
            hex!(
                "ff\
ff33\
ffa02222222222222222222222222222222222222222222222222222222222222222\
ff01\
80\
80"
            )
            .to_vec(),
        );
        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: Signature::default(),
        };
        validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            236,
            &Arc::new(BlsCache::default()),
        )
        .expect("SpendBundle should be valid for this test");
    }

    #[test]
    fn test_validate_unsafe() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            hex!("3333333333333333333333333333333333333333333333333333333333333333").into(),
            1,
        );

        let solution = hex!("ffff31ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080").to_vec();
        // ((49 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());
        let msg = b"hello";
        let sig = sign(&sk, msg);
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: sig,
        };
        validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            236,
            &Arc::new(BlsCache::default()),
        )
        .expect("SpendBundle should be valid for this test");
    }

    #[test]
    fn test_go_over_cost() {
        let test_coin = Coin::new(
            hex!("9dcf97a184f32623d11a73124ceb99a5709b083721e878a16d78f596718ba7b2").into(),
            hex!("9dcf97a184f32623d11a73124ceb99a5709b083721e878a16d78f596718ba7b2").into(),
            1_000_000_000,
        );
        let my_str = include_str!("large_spendbundle_validation_test.clsp.hex");
        let solution = hex::decode(my_str).expect("loading known file");
        // this solution makes 2400 CREATE_COIN conditions

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());

        let coin_spends: Vec<CoinSpend> = vec![spend.clone()];

        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: G2Element::default(),
        };
        let result = validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm / 2, // same as mempool_manager default
            &TEST_CONSTANTS,
            236,
            &Arc::new(BlsCache::default()),
        );
        assert!(matches!(result, Ok(..)));
        let result = validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm / 3, // lower than mempool_manager default
            &TEST_CONSTANTS,
            236,
            &Arc::new(BlsCache::default()),
        );
        assert!(matches!(result, Err(ErrorCode::CostExceeded)));
    }

    #[test]
    fn test_validate_aggsig_me() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        let full_puz = Bytes32::new(tree_hash_atom(&[1_u8]).to_bytes());
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            full_puz,
            1,
        );

        let solution = hex!("ffff32ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080").to_vec();
        // ((50 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());
        let msg = b"hello";
        let mut result = msg.to_vec();
        result.extend(
            [
                test_coin.coin_id().as_slice(),
                TEST_CONSTANTS.agg_sig_me_additional_data.as_slice(),
            ]
            .concat(),
        );
        let sig = sign(&sk, result.as_slice());
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: sig,
        };
        validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            1,
            &Arc::new(BlsCache::default()),
        )
        .expect("SpendBundle should be valid for this test");
    }

    #[test]
    fn test_validate_aggsig_parent_puzzle() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        //let pk: PublicKey = sk.public_key(); //0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2
        // panic!("{:?}", pk);

        let full_puz = Bytes32::new(tree_hash_atom(&[1_u8]).to_bytes());
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            full_puz,
            1,
        );

        let solution = hex!("ffff30ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080").to_vec();
        // ((48 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let spend = CoinSpend::new(
            test_coin,
            Program::new(vec![1_u8].into()),
            Program::new(solution.into()),
        );
        let msg = b"hello";
        let mut result = msg.to_vec();
        result.extend(
            [
                test_coin.parent_coin_info.as_slice(),
                test_coin.puzzle_hash.as_slice(),
                TEST_CONSTANTS
                    .agg_sig_parent_puzzle_additional_data
                    .as_slice(),
            ]
            .concat(),
        );
        let sig = sign(&sk, result.as_slice());
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: sig,
        };
        validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            TEST_CONSTANTS.hard_fork_height + 1,
            &Arc::new(BlsCache::default()),
        )
        .expect("SpendBundle should be valid for this test");
    }

    #[test]
    fn test_validate_aggsig_parent_amount() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        let full_puz = Bytes32::new(tree_hash_atom(&[1_u8]).to_bytes());
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            full_puz,
            1,
        );

        let solution = hex!("ffff2fffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080").to_vec();
        // ((47 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());
        let msg = b"hello";
        let mut result = msg.to_vec();
        result.extend(
            [
                test_coin.parent_coin_info.as_slice(),
                u64_to_bytes(test_coin.amount).as_slice(),
                TEST_CONSTANTS
                    .agg_sig_parent_amount_additional_data
                    .as_slice(),
            ]
            .concat(),
        );
        let sig = sign(&sk, result.as_slice());
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: sig,
        };
        validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            TEST_CONSTANTS.hard_fork_height + 1,
            &Arc::new(BlsCache::default()),
        )
        .expect("SpendBundle should be valid for this test");
    }

    #[test]
    fn test_validate_aggsig_puzzle_amount() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();

        let full_puz = Bytes32::new(tree_hash_atom(&[1_u8]).to_bytes());
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            full_puz,
            1,
        );

        let solution = hex!("ffff2effb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080").to_vec();
        // ((46 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());
        let msg = b"hello";
        let mut result = msg.to_vec();
        result.extend(
            [
                test_coin.puzzle_hash.as_slice(),
                u64_to_bytes(test_coin.amount).as_slice(),
                TEST_CONSTANTS
                    .agg_sig_puzzle_amount_additional_data
                    .as_slice(),
            ]
            .concat(),
        );
        let sig = sign(&sk, result.as_slice());
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: sig,
        };
        validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            TEST_CONSTANTS.hard_fork_height + 1,
            &Arc::new(BlsCache::default()),
        )
        .expect("SpendBundle should be valid for this test");
    }
}
