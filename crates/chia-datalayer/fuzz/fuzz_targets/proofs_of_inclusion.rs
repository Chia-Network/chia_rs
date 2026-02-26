#![no_main]

use chia_datalayer::{Error, Hash, InsertLocation, KeyId, MerkleBlob, ValueId};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|args: Vec<(KeyId, ValueId, Hash)>| {
    let mut blob = MerkleBlob::new(Vec::new()).expect("construct MerkleBlob");
    blob.check_integrity_on_drop = false;

    let mut keys: Vec<KeyId> = Vec::new();

    for (key, value, hash) in &args {
        match blob.insert(*key, *value, hash, InsertLocation::Auto {}) {
            Ok(_) => {
                keys.push(*key);
            }
            // should remain valid through these errors
            Err(Error::KeyAlreadyPresent()) => continue,
            Err(Error::HashAlreadyPresent()) => continue,
            // other errors should not be occurring
            Err(error) => panic!("unexpected error while inserting: {:?}", error),
        };
    }

    blob.calculate_lazy_hashes().unwrap();
    blob.check_integrity().unwrap();

    for key in keys {
        let proof = blob.get_proof_of_inclusion(key).unwrap();
        assert!(proof.valid());
    }
});
