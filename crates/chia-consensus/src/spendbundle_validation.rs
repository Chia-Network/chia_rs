use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
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

pub fn get_flags_for_height_and_constants(_height: u32, _constants: &ConsensusConstants) -> u32 {
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

    // The soft fork initiated with 2.5.0. The activation date is still TBD.
    // Adds a new keccak256 operator under the softfork guard with extension 1.
    // This operator can be hard forked in later, but is not included in a hard fork yet.

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::gen::make_aggsig_final_message::u64_to_bytes;
    use chia_bls::{sign, G2Element, PublicKey, SecretKey, Signature};
    use chia_protocol::{Coin, CoinSpend, Program};
    use clvm_utils::tree_hash_atom;
    use hex::FromHex;
    use hex_literal::hex;
    use rstest::rstest;

    fn mk_spend(puzzle: &[u8], solution: &[u8]) -> CoinSpend {
        let ph = tree_hash_atom(puzzle).to_bytes();
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            ph.into(),
            1_000_000_000,
        );
        CoinSpend::new(test_coin, Program::new(puzzle.into()), solution.into())
    }

    fn keys() -> (PublicKey, SecretKey) {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        (sk.public_key(), sk)
    }

    fn mk_agg_sig_solution(cond: u8, pk: &PublicKey) -> Vec<u8> {
        // ((<cond> <pk> "hello"))
        [
            hex!("ffff").as_slice(),
            &cond.to_be_bytes(),
            hex!("ffb0").as_slice(),
            pk.to_bytes().as_slice(),
            hex!("ff8568656c6c6f8080").as_slice(),
        ]
        .concat()
    }

    fn mk_agg_sig(cond: u8, sk: &SecretKey, spend: &CoinSpend, msg: &[u8]) -> Signature {
        let msg = match cond {
            46 => [
                msg,
                spend.coin.puzzle_hash.as_slice(),
                u64_to_bytes(spend.coin.amount).as_slice(),
                TEST_CONSTANTS
                    .agg_sig_puzzle_amount_additional_data
                    .as_slice(),
            ]
            .concat(),
            47 => [
                msg,
                spend.coin.parent_coin_info.as_slice(),
                u64_to_bytes(spend.coin.amount).as_slice(),
                TEST_CONSTANTS
                    .agg_sig_parent_amount_additional_data
                    .as_slice(),
            ]
            .concat(),
            48 => [
                msg,
                spend.coin.parent_coin_info.as_slice(),
                spend.coin.puzzle_hash.as_slice(),
                TEST_CONSTANTS
                    .agg_sig_parent_puzzle_additional_data
                    .as_slice(),
            ]
            .concat(),
            49 => msg.to_vec(),
            50 => [
                msg,
                spend.coin.coin_id().as_slice(),
                TEST_CONSTANTS.agg_sig_me_additional_data.as_slice(),
            ]
            .concat(),
            _ => panic!("unexpected"),
        };
        sign(&sk, msg.as_slice())
    }

    #[rstest]
    #[case(0, 0)]
    #[case(TEST_CONSTANTS.hard_fork_height, 0)]
    #[case(5_716_000, 0)]
    fn test_get_flags(#[case] height: u32, #[case] expected_value: u32) {
        assert_eq!(
            get_flags_for_height_and_constants(height, &TEST_CONSTANTS),
            expected_value
        );
    }

    #[test]
    fn test_validate_no_pks() {
        let solution = hex!(
            "ff\
ff33\
ffa02222222222222222222222222222222222222222222222222222222222222222\
ff01\
80\
80"
        );
        let spend = mk_spend(&[1_u8], &solution);
        let spend_bundle = SpendBundle {
            coin_spends: vec![spend],
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
    fn test_go_over_cost() {
        use std::fs::read_to_string;
        let my_str =
            read_to_string("../../generator-tests/large_spendbundle_validation_test.clsp.hex")
                .expect("test file not found");
        let solution = hex::decode(my_str).expect("parsing hex");
        // this solution makes 2400 CREATE_COIN conditions

        let spend = mk_spend(&[1_u8], &solution);

        let spend_bundle = SpendBundle {
            coin_spends: vec![spend],
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

    #[rstest]
    fn test_validate_agg_sig(#[values(46, 47, 48, 49, 50)] cond: u8) {
        let (pk, sk) = keys();

        // ((<cond> <pk> "hello"))
        let solution = mk_agg_sig_solution(cond, &pk);

        let spend = mk_spend(&[1_u8], &solution);
        let sig = mk_agg_sig(cond, &sk, &spend, b"hello");
        let spend_bundle = SpendBundle {
            coin_spends: vec![spend],
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

    #[rstest]
    fn test_failures(
        #[values(46, 47, 48, 49, 50)] condition_code: u8,
        #[values(0, 1)] whats_wrong: u8,
    ) {
        let (pk, sk) = keys();
        let solution = mk_agg_sig_solution(condition_code, &pk);
        let msg = if whats_wrong == 0 {
            b"goodbye".as_slice()
        } else {
            b"hello".as_slice()
        };
        let sk = if whats_wrong == 1 {
            SecretKey::from_bytes(
                &<[u8; 32]>::from_hex(
                    "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efc",
                )
                .unwrap(),
            )
            .unwrap()
        } else {
            sk
        };
        let spend = mk_spend(&[1_u8], &solution);
        let sig = mk_agg_sig(condition_code, &sk, &spend, msg);
        let spend_bundle = SpendBundle {
            coin_spends: vec![spend],
            aggregated_signature: sig,
        };
        let result = validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            246,
        );
        assert!(matches!(result, Err(ErrorCode::BadAggregateSignature)));
    }
}
