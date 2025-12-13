#![no_main]

use libfuzzer_sys::{arbitrary::Unstructured, fuzz_target};

use chia_datalayer::{BLOCK_SIZE, MerkleBlob};

fuzz_target!(|data: &[u8]| {
    let mut unstructured = Unstructured::new(data);
    let block_count = (unstructured.len() + (BLOCK_SIZE / 2)) / BLOCK_SIZE;
    let mut bytes = vec![0u8; block_count * BLOCK_SIZE];
    unstructured.fill_buffer(&mut bytes).unwrap();

    let Ok(mut blob) = MerkleBlob::new(bytes) else {
        return;
    };
    blob.check_integrity_on_drop = false;
    blob.check_integrity().unwrap();
});
