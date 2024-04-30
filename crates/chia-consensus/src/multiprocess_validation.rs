
use std::thread;
use std::sync::{Arc, Mutex};
use crate::{ConsensusConstants, BlockGenerator};
use crate::gen::errors::Err;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use chia_protocol::SpendBundle;
use crate::BlockGenerator;
use crate::gen::solution_generator::solution_generator;
use chia_protocol::Program;
use crate::gen::flags::{
    AGG_SIG_ARGS, NO_RELATIVE_CONDITIONS_ON_EPHEMERAL, ALLOW_BACKREFS, 
    ENABLE_SOFTFORK_CONDITION, ENABLE_MESSAGE_CONDITIONS
};
use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV};

// currently in multiprocess_validation.py
// called via blockchain.py from full_node.py when a full node wants to add a block or batch of blocks
fn pre_validate_blocks_multiprocessing() {

}

// currently in multiprocess_validation.py
// called in threads from pre_validate_blocks_multiprocessing
fn batch_pre_validate_blocks() {

}

// currently in mempool_manager.py
// called in full_node.py when adding a transaction
fn pre_validate_spendbundle() {

}

// currently in mempool_manager.py
// called in threads from pre_validate_spend_bundle()
// returns (error, cached_results, new_cache_entries, duration)
fn validate_clvm_and_signature(
    spend_bundle_bytes: bytes, 
    max_cost: int, 
    constants: ConsensusConstants, 
    height: uint32
) -> (Option<Err>, Vec<u8>, HashMap<[u8; 32], Vec<u8>> , u128) {
    let start_time = Instant::now();
    let additional_data = constants.AGG_SIG_ME_ADDITIONAL_DATA;
    let bundle = SpendBundle::from_bytes(spend_bundle_bytes);
    let solution = simple_solution_generator(bundle);
    return (None, start.elapsed());
}

pub fn simple_solution_generator(bundle: SpendBundle) -> BlockGenerator {
    let mut spends = Vec<(Coin, &[u8], &[u8])>::new();
    for cs in bundle.coin_spends {
        spends.append((cs.coin, cs.puzzle_reveal.to_bytes(), cs.solution.to_bytes()));
    }
    let block_program = solution_generator(spends);
    BlockGenerator(Program.from_bytes(block_program), &[], &[])
}

pub fn get_flags_for_height_and_constants(height: u32, constats: ConsensusConstants) -> u32 {
    let mut flags: u32 = 0;
    if height >= constants.SOFT_FORK2_HEIGHT{
        flags = flags | NO_RELATIVE_CONDITIONS_ON_EPHEMERAL
    }
    if height >= constants.SOFT_FORK4_HEIGHT{
        flags = flags | ENABLE_MESSAGE_CONDITIONS
    }
    if height >= constants.HARD_FORK_HEIGHT {
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
        flags = (
            flags
            | ENABLE_SOFTFORK_CONDITION
            | ENABLE_BLS_OPS_OUTSIDE_GUARD
            | ENABLE_FIXED_DIV
            | AGG_SIG_ARGS
            | ALLOW_BACKREFS
        )
    }
    flags
}