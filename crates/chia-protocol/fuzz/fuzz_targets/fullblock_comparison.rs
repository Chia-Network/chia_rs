#![no_main]

use chia_protocol::FullBlock;
use chia_traits::Streamable;
use libfuzzer_sys::fuzz_target;

/// Fuzz target that validates deferred FullBlock roundtrips and lazy generator
/// parsing don't panic for arbitrary inputs.
fuzz_target!(|data: &[u8]| {
    let result = FullBlock::from_bytes(data);

    if let Ok(block) = result {
        let _ = block.transactions_generator();
        let _ = block.transactions_generator_ref_list();
        let bytes = block.to_bytes().expect("FullBlock serialization failed");
        assert_eq!(bytes, data, "FullBlock serialization roundtrip mismatch");
    }
});
