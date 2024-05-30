use crate::consensus_constants::ConsensusConstants;
use crate::gen::condition_tools::pkm_pairs;
use crate::gen::flags::{
    AGG_SIG_ARGS, ALLOW_BACKREFS, ENABLE_MESSAGE_CONDITIONS, ENABLE_SOFTFORK_CONDITION,
    NO_RELATIVE_CONDITIONS_ON_EPHEMERAL,
};
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::solution_generator::solution_generator;
use crate::gen::validation_error::ErrorCode;
use crate::generator_types::BlockGenerator;
use crate::npc_result::get_name_puzzle_conditions;
use chia_bls::aggregate_verify;
use chia_bls::BlsCache;
use chia_bls::PublicKey;
use chia_protocol::Coin;
use chia_protocol::Program;
use chia_protocol::SpendBundle;
use chia_traits::Streamable;
use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// currently in multiprocess_validation.py
// called via blockchain.py from full_node.py when a full node wants to add a block or batch of blocks
fn pre_validate_blocks_multiprocessing() {}

// currently in multiprocess_validation.py
// called in threads from pre_validate_blocks_multiprocessing
fn batch_pre_validate_blocks() {}

// currently in mempool_manager.py
// called in full_node.py when adding a transaction
pub fn pre_validate_spendbundle(
    new_spend: SpendBundle,
    max_cost: u64,
    constants: ConsensusConstants,
    peak_height: u32,
    syncing: bool,
    cache: Arc<Mutex<BlsCache>>,
) -> Result<OwnedSpendBundleConditions, ErrorCode> {
    if new_spend.coin_spends.is_empty() {
        Err(ErrorCode::InvalidSpendBundle)
    } else {
        let (result, duration) = validate_clvm_and_signature(
            &new_spend,
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
    constants: ConsensusConstants,
    height: u32,
    syncing: bool,
    cache: Arc<Mutex<BlsCache>>,
) -> Result<(OwnedSpendBundleConditions, Duration), ErrorCode> {
    let start_time = Instant::now();
    let additional_data = constants.agg_sig_me_additional_data;
    let program: BlockGenerator = simple_solution_generator(&spend_bundle)?;
    let npcresult = get_name_puzzle_conditions(program, max_cost, true, height, constants);
    match npcresult {
        Err(e) => return Err(e),
        Ok(unwrapped) => {
            let npcresult = unwrapped;
            let (pks, msgs) = pkm_pairs(npcresult.clone(), additional_data.as_slice())?;
            // Verify aggregated signature
            if !{
                if syncing {
                    // if we're syncing use the chia_bls::aggregate_verify to avoid using the cache
                    let data: Vec<(&PublicKey, &[u8])> = pks
                        .iter()
                        .zip(msgs.iter().map(|msg| msg.as_slice()))
                        .collect();
                    aggregate_verify(&spend_bundle.aggregated_signature, data)
                } else {
                    // if we're fully synced then use the cache
                    cache.lock().unwrap().aggregate_verify(
                        pks,
                        msgs,
                        &spend_bundle.aggregated_signature,
                    )
                }
            } {
                return Err(ErrorCode::InvalidSpendBundle);
            }
            Ok((npcresult, start_time.elapsed()))
        }
    }
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

pub fn simple_solution_generator(bundle: &SpendBundle) -> Result<BlockGenerator, ErrorCode> {
    let mut spends = Vec::<(Coin, Vec<u8>, Vec<u8>)>::new();
    for cs in &bundle.coin_spends {
        let puzzle_reveal = cs.puzzle_reveal.to_vec();
        let solution_reveal = cs.solution.to_vec();
        spends.push((cs.coin, puzzle_reveal, solution_reveal));
    }
    let block_program = solution_generator(spends);
    match block_program {
        Ok(bp) => {
            let program = Program::from_bytes(bp.as_slice());
            match program {
                Ok(p) => Ok(BlockGenerator {
                    program: p,
                    generator_refs: Vec::<Program>::new(),
                    block_height_list: Vec::<u32>::new(),
                }),
                Err(_) => Err(ErrorCode::InvalidSpendBundle),
            }
        }
        Err(_) => Err(ErrorCode::InvalidSpendBundle),
    }
}

pub fn get_flags_for_height_and_constants(height: u32, constants: ConsensusConstants) -> u32 {
    let mut flags: u32 = 0;
    if height >= constants.soft_fork2_height {
        flags = flags | NO_RELATIVE_CONDITIONS_ON_EPHEMERAL
    }
    if height >= constants.soft_fork4_height {
        flags = flags | ENABLE_MESSAGE_CONDITIONS
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
            | ALLOW_BACKREFS
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use chia_bls::Signature;
    use chia_protocol::CoinSpend;

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
            TEST_CONSTANTS,
            236,
            true,
            Arc::new(Mutex::new(BlsCache::default())),
        );
        result.unwrap();
    }
}
