use crate::consensus_constants::ConsensusConstants;
use crate::gen::condition_tools::make_aggsig_final_message;
use crate::gen::flags::{
    AGG_SIG_ARGS, ALLOW_BACKREFS, DISALLOW_INFINITY_G1, ENABLE_MESSAGE_CONDITIONS, ENABLE_SOFTFORK_CONDITION, NO_RELATIVE_CONDITIONS_ON_EPHEMERAL
};
use crate::gen::opcodes::{
    AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
};
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::validation_error::ErrorCode;
use crate::npc_result::get_name_puzzle_conditions;
use chia_bls::aggregate_verify;
use chia_bls::BlsCache;
use chia_protocol::SpendBundle;
use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV};
use std::sync::{Arc, Mutex};
// use std::thread;
use std::time::{Duration, Instant};

// currently in mempool_manager.py
// called in full_node.py when adding a transaction
pub fn pre_validate_spendbundle(
    new_spend: &SpendBundle,
    max_cost: u64,
    constants: &ConsensusConstants,
    peak_height: u32,
    syncing: bool,
    cache: Arc<Mutex<BlsCache>>,
) -> Result<OwnedSpendBundleConditions, ErrorCode> {
    if new_spend.coin_spends.is_empty() {
        Err(ErrorCode::InvalidSpendBundle)
    } else {
        let (result, _duration) = validate_clvm_and_signature(
            new_spend,
            max_cost,
            constants,
            peak_height,
            syncing,
            cache,
        )?;
        Ok(result)
    }
}

// currently in mempool_manager.py
// called in threads from pre_validate_spend_bundle()
// returns (error, cached_results, new_cache_entries, duration)
fn validate_clvm_and_signature(
    spend_bundle: &SpendBundle,
    max_cost: u64,
    constants: &ConsensusConstants,
    height: u32,
    syncing: bool,
    cache: Arc<Mutex<BlsCache>>,
) -> Result<(OwnedSpendBundleConditions, Duration), ErrorCode> {
    let start_time = Instant::now();
    let npcresult = get_name_puzzle_conditions(spend_bundle, max_cost, true, height, constants).map_err(|e| e.1)?;
    let iter = npcresult.spends.iter().flat_map(|spend| {
        // let spend_clone = spend.clone();
        let condition_items_pairs = vec![
            (AGG_SIG_PARENT, &spend.agg_sig_parent),
            (AGG_SIG_PUZZLE, &spend.agg_sig_puzzle),
            (AGG_SIG_AMOUNT, &spend.agg_sig_amount),
            (AGG_SIG_PUZZLE_AMOUNT, &spend.agg_sig_puzzle_amount),
            (AGG_SIG_PARENT_AMOUNT, &spend.agg_sig_parent_amount),
            (AGG_SIG_PARENT_PUZZLE, &spend.agg_sig_parent_puzzle),
            (AGG_SIG_ME, &spend.agg_sig_me),
        ];
        condition_items_pairs
            .iter()
            .flat_map(|(condition, items)| {
                let spend = spend.clone();
                items.iter().map(move |(pk, msg)| {
                    (
                        pk,
                        make_aggsig_final_message(*condition, msg.as_slice(), &spend, constants),
                    )
                })
            }).collect::<Vec<_>>()
    });
    let unsafe_items = npcresult.agg_sig_unsafe.iter().map(|(pk, msg)| {
        (
            pk,
            msg.as_slice().to_vec()
        )
    });
    let iter = iter.chain(unsafe_items);
    // Verify aggregated signature
    if !{
        if syncing {
            aggregate_verify(&spend_bundle.aggregated_signature, iter)
        } else {
            // if we're fully synced then use the cache
            cache
                .lock()
                .unwrap()
                .aggregate_verify_with_iter(iter, &spend_bundle.aggregated_signature)
        }
    } {
        return Err(ErrorCode::InvalidSpendBundle);
    }
    Ok((npcresult, start_time.elapsed()))
}

// #[cfg(feature = "py-bindings")]
// mod py_funcs {
//     use super::*;
//     use pyo3::{
//         exceptions::PyValueError,
//         pybacked::PyBackedBytes,
//         pyfunction,
//         types::{PyAnyMethods, PyList},
//         Bound, PyObject, PyResult,
//     };
//     use crate::gen::owned_conditions;

//     #[pyfunction]
//     #[pyo3(name = "pre_validate_spendbundle")]
//     pub fn py_pre_validate_spendbundle(
//         new_spend: SpendBundle,
//         max_cost: u64,
//         constants: ConsensusConstants,
//         peak_height: u32,
//         syncing: bool,
//         cache: BlsCache
//     ) -> Result<(SpendBundle, OwnedSpendBundleConditions), ErrorCode> {
//         let sbc = validate_clvm_and_signature(&new_spend, max_cost, constants, peak_height, syncing, Arc::new(Mutex::new(cache)));  // TODO: use cache properly
//         match sbc {
//             Ok(owned_conditions) => {
//                 Ok((new_spend, owned_conditions.0))
//             },
//             Err(e) => {
//                 Err(e)
//             }
//         }
//     }
// }

pub fn get_flags_for_height_and_constants(height: u32, constants: &ConsensusConstants) -> u32 {
    let mut flags: u32 = 0;
    if height >= constants.soft_fork2_height {
        flags |= NO_RELATIVE_CONDITIONS_ON_EPHEMERAL;
    }
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
        flags = flags
            | ENABLE_SOFTFORK_CONDITION
            | ENABLE_BLS_OPS_OUTSIDE_GUARD
            | ENABLE_FIXED_DIV
            | AGG_SIG_ARGS
            | ALLOW_BACKREFS;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use chia_bls::{SecretKey, Signature, sign};
    use chia_protocol::{Coin, CoinSpend, Program};
    use clvm_utils::tree_hash_atom;
    use chia_protocol::Bytes32;
    use hex::FromHex;

    #[test]
    fn test_validate_no_pks() {
        let test_coin = Coin::new(
            hex::decode("4444444444444444444444444444444444444444444444444444444444444444")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("3333333333333333333333333333333333333333333333333333333333333333")
                .unwrap()
                .try_into()
                .unwrap(),
            1,
        );
        let solution = "ff\
ff33\
ffa02222222222222222222222222222222222222222222222222222222222222222\
ff01\
80\
80";
        let solution = hex::decode(solution)
            .expect("hex::decode")
            .try_into()
            .unwrap();
        let spend = CoinSpend::new(
            test_coin,
            Program::new(vec![1_u8].into()),
            Program::new(solution),
        );
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends: coin_spends,
            aggregated_signature: Signature::default(),
        };
        let result = validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            236,
            true,
            Arc::new(Mutex::new(BlsCache::default())),
        );
        result.unwrap();
    }

    #[test]
    fn test_validate_unsafe() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        //let pk: PublicKey = sk.public_key(); //0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2
        // panic!("{:?}", pk);
        let test_coin = Coin::new(
            hex::decode("4444444444444444444444444444444444444444444444444444444444444444")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("3333333333333333333333333333333333333333333333333333333333333333")
                .unwrap()
                .try_into()
                .unwrap(),
            1,
        );

        let solution = "ffff31ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080";
        // ((49 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let solution = hex::decode(solution)
            .expect("hex::decode")
            .try_into()
            .unwrap();
        let spend = CoinSpend::new(
            test_coin,
            Program::new(vec![1_u8].into()),
            Program::new(solution),
        );
        let msg = b"hello";
        let sig = sign(&sk, msg);
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends: coin_spends,
            aggregated_signature: sig,
        };
        let result = validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            236,
            true,
            Arc::new(Mutex::new(BlsCache::default())),
        );
        match result{
            Ok(_) => return,
            Err(e) => panic!("{:?}", e)
        }
    }    

    #[test]
    fn test_validate_aggsig_me() {
        let sk_hex = "52d75c4707e39595b27314547f9723e5530c01198af3fc5849d9a7af65631efb";
        let sk = SecretKey::from_bytes(&<[u8; 32]>::from_hex(sk_hex).unwrap()).unwrap();
        //let pk: PublicKey = sk.public_key(); //0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2
        // panic!("{:?}", pk);

        let full_puz = Bytes32::new(tree_hash_atom(&[1_u8]).to_bytes());
        let test_coin = Coin::new(
            hex::decode("4444444444444444444444444444444444444444444444444444444444444444")
                .unwrap()
                .try_into()
                .unwrap(),
                full_puz,
            1,
        );

        let solution = "ffff32ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080";
        // ((50 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let solution = hex::decode(solution)
            .expect("hex::decode")
            .try_into()
            .unwrap();
        let spend = CoinSpend::new(
            test_coin,
            Program::new(vec![1_u8].into()),
            Program::new(solution),
        );
        let msg = b"hello";
        let mut result = msg.to_vec();
        result.extend([
            test_coin.coin_id().as_slice(),
            TEST_CONSTANTS.agg_sig_me_additional_data.as_slice(),
        ]
        .concat());
        let sig = sign(&sk, result.as_slice());
        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends: coin_spends,
            aggregated_signature: sig,
        };
        let result = validate_clvm_and_signature(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            &TEST_CONSTANTS,
            236,
            true,
            Arc::new(Mutex::new(BlsCache::default())),
        );
        match result{
            Ok(_) => return,
            Err(e) => panic!("{:?}", e)
        }
    }    
}
