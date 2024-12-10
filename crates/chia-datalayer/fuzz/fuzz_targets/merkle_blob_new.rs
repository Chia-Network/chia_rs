#![no_main]

use libfuzzer_sys::fuzz_target;

use chia_datalayer::MerkleBlob;

fuzz_target!(|data: &[u8]| {
    let _ = MerkleBlob::new(data.to_vec());
});
