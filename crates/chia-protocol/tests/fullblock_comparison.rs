/// Compare FullBlock and FullBlock2 parsing to ensure deferred parsing produces identical results.
///
/// This test validates that GeneratorInfo-based deferred parsing (FullBlock2) produces
/// the same logical results as the current eager parsing (FullBlock).

use chia_protocol::{FullBlock, FullBlock2};
use chia_traits::Streamable;

#[test]
fn test_fullblock2_basic() {
    // Just verify FullBlock2 compiles and basic API works
    // Real validation happens in the fuzzer and blockchain data tests
    
    // Create an arbitrary byte sequence
    let fake_block_bytes = vec![0u8; 100];
    
    // Try to parse (will likely fail, but that's okay)
    let result = FullBlock2::from_bytes(&fake_block_bytes);
    
    // Just checking that the API exists and compiles
    if let Ok(block) = result {
        let _ = block.height();
        let _ = block.transactions_generator();
        let _ = block.transactions_generator_ref_list();
    }
}

/// Helper to compare all fields between FullBlock and FullBlock2
fn compare_blocks(block1: &FullBlock, block2: &FullBlock2, context: &str) {
    assert_eq!(block1.height(), block2.height(), "{}: height mismatch", context);
    assert_eq!(block1.weight(), block2.weight(), "{}: weight mismatch", context);
    assert_eq!(block1.total_iters(), block2.total_iters(), "{}: total_iters mismatch", context);
    assert_eq!(block1.prev_header_hash(), block2.prev_header_hash(), "{}: prev_header_hash mismatch", context);
    assert_eq!(block1.header_hash(), block2.header_hash(), "{}: header_hash mismatch", context);
    assert_eq!(block1.is_transaction_block(), block2.is_transaction_block(), "{}: is_transaction_block mismatch", context);
    assert_eq!(block1.is_fully_compactified(), block2.is_fully_compactified(), "{}: is_fully_compactified mismatch", context);
    
    // Compare generator data
    let gen1_present = block1.transactions_generator.is_some();
    let gen2_present = block2.transactions_generator().unwrap().is_some();
    assert_eq!(gen1_present, gen2_present, "{}: generator presence mismatch", context);
    
    if gen1_present {
        let gen1 = block1.transactions_generator.as_ref().unwrap();
        let gen2 = block2.transactions_generator().unwrap().unwrap();
        assert_eq!(gen1.as_slice(), gen2.as_slice(), "{}: generator bytes mismatch", context);
    }
    
    // Compare ref lists
    let ref_list1 = &block1.transactions_generator_ref_list;
    let ref_list2 = block2.transactions_generator_ref_list().unwrap();
    assert_eq!(ref_list1, &ref_list2, "{}: ref_list mismatch", context);
}

/// Load and compare multiple blocks from test data if available
#[test]
#[ignore] // Only run when test data is available
fn test_compare_real_blockchain_blocks() {
    use std::fs;
    use std::path::Path;
    
    let test_data_dir = Path::new("test_data/blocks");
    if !test_data_dir.exists() {
        eprintln!("Skipping test - test_data/blocks directory not found");
        return;
    }
    
    let mut tested_count = 0;
    
    for entry in fs::read_dir(test_data_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) != Some("bin") {
            continue;
        }
        
        let block_bytes = fs::read(&path).unwrap();
        let context = format!("block file: {}", path.display());
        
        // Parse with both types
        let block1 = match FullBlock::from_bytes(&block_bytes) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Warning: Failed to parse {} with FullBlock: {}", path.display(), e);
                continue;
            }
        };
        
        let block2 = match FullBlock2::from_bytes(&block_bytes) {
            Ok(b) => b,
            Err(e) => {
                panic!("FullBlock2 failed to parse {} (but FullBlock succeeded): {}", path.display(), e);
            }
        };
        
        compare_blocks(&block1, &block2, &context);
        tested_count += 1;
    }
    
    println!("Successfully compared {} blocks", tested_count);
    assert!(tested_count > 0, "No blocks were tested");
}
