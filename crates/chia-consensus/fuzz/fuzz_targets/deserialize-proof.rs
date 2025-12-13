#![no_main]
use chia_consensus::merkle_tree::{MerkleSet, validate_merkle_proof};
use hex_literal::hex;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _r = MerkleSet::from_proof(data);
    let dummy: [u8; 32] = hex!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    assert!(!matches!(
        validate_merkle_proof(data, &dummy, &dummy),
        Ok(true)
    ));
});
