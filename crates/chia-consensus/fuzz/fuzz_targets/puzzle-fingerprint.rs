#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::puzzle_fingerprint::compute_puzzle_fingerprint;
use chia_protocol::Program;
use clvmr::serde::node_from_bytes;
use clvmr::serde::serialized_length_from_bytes_trusted;
use clvmr::Allocator;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let Ok(_puzzle_node) = node_from_bytes(&mut a, data) else {
        return;
    };

    let len = serialized_length_from_bytes_trusted(data)
        .expect("serialized_length_from_bytes_trusted") as usize;
    let puzzle = Program::new(data[0..len].into());

    let _ = compute_puzzle_fingerprint(
        &puzzle,
        &Program::default(),
        TEST_CONSTANTS.max_block_cost_clvm,
        0,
    );
});
