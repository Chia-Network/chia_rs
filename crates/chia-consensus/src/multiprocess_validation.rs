
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