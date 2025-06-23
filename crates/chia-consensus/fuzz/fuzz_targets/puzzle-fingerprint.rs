#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_bls::Signature;
use chia_consensus::conditions::ELIGIBLE_FOR_DEDUP;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::DONT_VALIDATE_SIGNATURE;
use chia_consensus::puzzle_fingerprint::compute_puzzle_fingerprint;
use chia_consensus::run_block_generator::run_block_generator2;
use chia_consensus::solution_generator::solution_generator_backrefs;
use chia_protocol::{Coin, Program};
use clvm_utils::tree_hash_from_bytes;
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

    let fingerprint_result = compute_puzzle_fingerprint(
        &puzzle,
        &Program::default(),
        TEST_CONSTANTS.max_block_cost_clvm,
        0,
    );

    // compute_puzzle_fingerprint() doesn't validate the puzzle, it
    // assume's it's valid. run_block_generator2() does validate it, so it will
    // fail in case it turned out to be invalid
    let dummy_coin = Coin {
        parent_coin_info: b"00000000000000000000000000000000".into(),
        puzzle_hash: tree_hash_from_bytes(puzzle.as_ref())
            .expect("tree_hash")
            .into(),
        amount: 100,
    };

    let generator = solution_generator_backrefs([(dummy_coin, puzzle, Program::default())])
        .expect("solution_generator");

    let blocks: &[&[u8]] = &[];
    let Ok(block_conds) = run_block_generator2(
        &mut a,
        &generator,
        blocks,
        11_000_000_000,
        DONT_VALIDATE_SIGNATURE,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    ) else {
        return;
    };

    assert_eq!(block_conds.spends.len(), 1);

    // if the spend is not eligible for dedup, compute_puzzle_fingerprint()
    // may or may not fail. It doesn't perform full validation
    if (block_conds.spends[0].flags & ELIGIBLE_FOR_DEDUP) != 0 {
        let Ok((cost, _fingerprint)) = fingerprint_result else {
            panic!("run_block_generator2() passed and is eligible for dedup, but compute_puzzle_fingerprint() failed");
        };

        // running the block has higher cost than the puzzle, because it includes
        // the cost of the quote, which is 20
        let expect_cost = block_conds.execution_cost + block_conds.condition_cost - 20;

        assert_eq!(expect_cost, cost);
    }
});
