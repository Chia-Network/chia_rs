#![no_main]

use chia_protocol::{FullBlock, FullBlock2};
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

/// Fuzz target that validates FullBlock and FullBlock2 produce identical results.
///
/// This fuzzer:
/// 1. Attempts to parse input bytes as both FullBlock and FullBlock2
/// 2. If both succeed, verifies they produce identical logical results
/// 3. If both fail, that's also fine (invalid input)
/// 4. Panic if one succeeds but the other fails (inconsistency!)
///
/// This validates that GeneratorInfo-based deferred parsing has no observable
/// difference from the current eager parsing approach.
fuzz_target!(|data: &[u8]| {
    let result1 = FullBlock::from_bytes(data);
    let result2 = FullBlock2::from_bytes(data);

    match (result1, result2) {
        (Ok(block1), Ok(block2)) => {
            // Both succeeded - verify they agree on everything
            
            // Basic fields
            assert_eq!(block1.height(), block2.height(), "height mismatch");
            assert_eq!(block1.weight(), block2.weight(), "weight mismatch");
            assert_eq!(block1.total_iters(), block2.total_iters(), "total_iters mismatch");
            assert_eq!(block1.prev_header_hash(), block2.prev_header_hash(), "prev_header_hash mismatch");
            assert_eq!(block1.header_hash(), block2.header_hash(), "header_hash mismatch");
            assert_eq!(block1.is_transaction_block(), block2.is_transaction_block(), "is_transaction_block mismatch");
            assert_eq!(block1.is_fully_compactified(), block2.is_fully_compactified(), "is_fully_compactified mismatch");
            
            // Generator data
            let gen1_present = block1.transactions_generator.is_some();
            let gen2_result = block2.transactions_generator();
            
            // If block2 parsing fails, that's a bug since block1 succeeded
            let gen2_present = gen2_result.expect("FullBlock2.transactions_generator() failed but FullBlock succeeded").is_some();
            
            assert_eq!(gen1_present, gen2_present, "generator presence mismatch");
            
            if gen1_present {
                let gen1 = block1.transactions_generator.as_ref().unwrap();
                let gen2 = block2.transactions_generator().unwrap().unwrap();
                assert_eq!(gen1.as_slice(), gen2.as_slice(), "generator bytes mismatch");
            }
            
            // Ref lists
            let ref_list1 = &block1.transactions_generator_ref_list;
            let ref_list2 = block2.transactions_generator_ref_list()
                .expect("FullBlock2.transactions_generator_ref_list() failed but FullBlock succeeded");
            assert_eq!(ref_list1, &ref_list2, "ref_list mismatch");
            
            // Serialization roundtrip - both should produce identical bytes
            let bytes1 = block1.to_bytes().expect("FullBlock serialization failed");
            let bytes2 = block2.to_bytes().expect("FullBlock2 serialization failed");
            assert_eq!(bytes1, bytes2, "serialization mismatch - FullBlock and FullBlock2 produce different bytes");
        }
        (Err(_), Err(_)) => {
            // Both failed - that's fine, invalid input
        }
        (Ok(_), Err(e)) => {
            panic!("FullBlock succeeded but FullBlock2 failed: {:?}", e);
        }
        (Err(e), Ok(_)) => {
            panic!("FullBlock2 succeeded but FullBlock failed: {:?}", e);
        }
    }
});
