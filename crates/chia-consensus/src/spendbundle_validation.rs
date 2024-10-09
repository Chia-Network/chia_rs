use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
use crate::gen::flags::ALLOW_BACKREFS;
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::validation_error::ErrorCode;
use crate::spendbundle_conditions::run_spendbundle;
use chia_bls::GTElement;
use chia_bls::{aggregate_verify_gt, hash_to_g2};
use chia_protocol::SpendBundle;
use chia_sha2::Sha256;
use clvmr::LIMIT_HEAP;
use std::time::{Duration, Instant};

// type definition makes clippy happy
pub type ValidationPair = ([u8; 32], GTElement);

// currently in mempool_manager.py
// called in threads from pre_validate_spend_bundle()
// pybinding returns (error, cached_results, new_cache_entries, duration)
pub fn validate_clvm_and_signature(
    spend_bundle: &SpendBundle,
    max_cost: u64,
    constants: &ConsensusConstants,
    height: u32,
) -> Result<(OwnedSpendBundleConditions, Vec<ValidationPair>, Duration), ErrorCode> {
    let start_time = Instant::now();
    let mut a = make_allocator(LIMIT_HEAP);
    let (sbc, pkm_pairs) =
        run_spendbundle(&mut a, spend_bundle, max_cost, height, 0, constants).map_err(|e| e.1)?;
    let conditions = OwnedSpendBundleConditions::from(&a, sbc);

    // Collect all pairs in a single vector to avoid multiple iterations
    let mut pairs = Vec::new();

    let mut aug_msg = Vec::<u8>::new();

    for (pk, msg) in pkm_pairs {
        aug_msg.clear();
        aug_msg.extend_from_slice(&pk.to_bytes());
        aug_msg.extend(&*msg);
        let aug_hash = hash_to_g2(&aug_msg);
        let pairing = aug_hash.pair(&pk);

        let mut key = Sha256::new();
        key.update(&aug_msg);
        pairs.push((key.finalize(), pairing));
    }
    // Verify aggregated signature
    let result = aggregate_verify_gt(
        &spend_bundle.aggregated_signature,
        pairs.iter().map(|tuple| &tuple.1),
    );
    if !result {
        return Err(ErrorCode::BadAggregateSignature);
    }

    // Collect results
    Ok((conditions, pairs, start_time.elapsed()))
}

pub fn get_flags_for_height_and_constants(height: u32, constants: &ConsensusConstants) -> u32 {
    let mut flags: u32 = 0;

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
        flags |= ALLOW_BACKREFS;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::gen::make_aggsig_final_message::u64_to_bytes;
    use chia_bls::{sign, G2Element, SecretKey, Signature};
    use chia_protocol::{Bytes, Bytes32};
    use chia_protocol::{Coin, CoinSpend, Program};
    use clvm_utils::tree_hash_atom;
    use hex::FromHex;
    use hex_literal::hex;
    use rstest::rstest;

    #[rstest]
    #[case(0, 0)]
    #[case(TEST_CONSTANTS.hard_fork_height, ALLOW_BACKREFS)]
    #[case(5_716_000, ALLOW_BACKREFS)]
    fn test_get_flags(#[case] height: u32, #[case] expected_value: u32) {
        assert_eq!(
            get_flags_for_height_and_constants(height, &TEST_CONSTANTS),
            expected_value
        );
    }

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
        )
        .expect("SpendBundle should be valid for this test");
    }

    #[test]
    fn test_go_over_cost() {
        use std::fs::read_to_string;
        let test_coin = Coin::new(
            hex!("9dcf97a184f32623d11a73124ceb99a5709b083721e878a16d78f596718ba7b2").into(),
            hex!("9dcf97a184f32623d11a73124ceb99a5709b083721e878a16d78f596718ba7b2").into(),
            1_000_000_000,
        );
        let my_str =
            read_to_string("../../generator-tests/large_spendbundle_validation_test.clsp.hex")
                .expect("test file not found");
        let solution = hex::decode(my_str).expect("parsing hex");
        // this solution makes 2400 CREATE_COIN conditions

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());

        let coin_spends: Vec<CoinSpend> = vec![spend.clone()];

        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: G2Element::default(),
        };
        let expected_cost = 5_527_116_044;
        let max_cost = expected_cost;
        let test_height = 236;
        let (conds, _, _) =
            validate_clvm_and_signature(&spend_bundle, max_cost, &TEST_CONSTANTS, test_height)
                .expect("validate_clvm_and_signature failed");
        assert_eq!(conds.cost, expected_cost);
        let result =
            validate_clvm_and_signature(&spend_bundle, max_cost - 1, &TEST_CONSTANTS, test_height);
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
        )
        .expect("SpendBundle should be valid for this test");
    }
}
