#![no_main]

use libfuzzer_sys::{arbitrary::Unstructured, fuzz_target};

use chia_datalayer::{merkle::dot::open_dot, MerkleBlob, BLOCK_SIZE};

fuzz_target!(|data: &[u8]| {
    let mut unstructured = Unstructured::new(data);
    let block_count = unstructured.int_in_range(1..=1000).unwrap();
    let mut bytes = vec![0u8; block_count * BLOCK_SIZE];
    unstructured.fill_buffer(&mut bytes).unwrap();

    println!("bytes: {:?}", bytes);
    let Ok(mut blob) = MerkleBlob::new(bytes) else {
        return;
    };
    open_dot(&mut blob.to_dot().unwrap());

    blob.check_integrity_on_drop = false;
    blob.check_integrity().unwrap();
});
