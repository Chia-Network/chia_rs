#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::merkle_tree::MerkleSet;

fuzz_target!(|data: &[u8]| {
    let _r = MerkleSet::from_proof(data);
});
