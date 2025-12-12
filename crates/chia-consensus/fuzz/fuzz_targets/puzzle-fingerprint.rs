#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::puzzle_fingerprint::compute_puzzle_fingerprint;
use clvmr::Allocator;
use clvmr::serde::node_from_bytes;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let Ok(conditions) = node_from_bytes(&mut a, data) else {
        return;
    };

    let _ = compute_puzzle_fingerprint(&a, conditions);
});
