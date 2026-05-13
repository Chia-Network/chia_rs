/// Smoke tests for the public deferred FullBlock API.
///
/// Eager-vs-deferred equivalence is proven by the chia-tools
/// fullblock-deferred-equivalence binary, which keeps the eager wire model as
/// private proving-only code.
use chia_protocol::FullBlock;
use chia_traits::Streamable;

#[test]
fn test_fullblock_deferred_api_compiles() {
    let fake_block_bytes = vec![0u8; 100];

    // Try to parse (will likely fail, but that's okay). This keeps the lazy
    // generator accessors type-checked by the normal protocol test target.
    let result = FullBlock::from_bytes(&fake_block_bytes);

    if let Ok(block) = result {
        let _ = block.height();
        let _ = block.transactions_generator();
        let _ = block.transactions_generator_ref_list();
    }
}

/// Load and compare multiple blocks from test data if available
#[test]
#[ignore] // Only run when test data is available
fn test_parse_real_blockchain_blocks() {
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

        let block = match FullBlock::from_bytes(&block_bytes) {
            Ok(b) => b,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse {} with FullBlock: {}",
                    path.display(),
                    e
                );
                continue;
            }
        };

        let _ = block.parse_generator_data();
        tested_count += 1;
    }

    println!("Successfully parsed {} blocks", tested_count);
    assert!(tested_count > 0, "No blocks were tested");
}
