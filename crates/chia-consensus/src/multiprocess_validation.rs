
use std::thread;
use std::sync::{Arc, Mutex};
use crate::ConsensusConstants;
use crate::gen::errors::Err;
use std::collections::HashMap;
use std::time::{Duration, Instant};


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
    duration = Instant::now();

    return (None, start.elapsed());
}