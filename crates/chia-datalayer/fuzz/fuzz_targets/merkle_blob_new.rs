#![no_main]

use libfuzzer_sys::{arbitrary::Unstructured, fuzz_target};

use chia_datalayer::{MerkleBlob, BLOCK_SIZE};

fuzz_target!(|data: &[u8]| {
    let mut unstructured = Unstructured::new(data);
    let block_count = unstructured.int_in_range(0..=1000).unwrap();
    let mut bytes = vec![0u8; block_count * BLOCK_SIZE];
    unstructured.fill_buffer(&mut bytes).unwrap();

    let Ok(mut blob) = MerkleBlob::new(bytes) else {
        return;
    };
    blob.check_integrity_on_drop = false;
});
