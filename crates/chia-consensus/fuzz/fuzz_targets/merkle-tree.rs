#![no_main]
use chia_consensus::merkle_tree::MerkleSet;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = MerkleSet::from_proof(data);
});
