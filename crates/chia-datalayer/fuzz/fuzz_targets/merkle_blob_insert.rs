#![no_main]

use chia_datalayer::{Error, Hash, InsertLocation, KeyId, MerkleBlob, ValueId};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|args: Vec<(KeyId, ValueId, Hash)>| {
    let mut blob = MerkleBlob::new(Vec::new()).expect("construct MerkleBlob");
    blob.check_integrity_on_drop = false;

    for (key, value, hash) in &args {
        match blob.insert(*key, *value, hash, InsertLocation::Auto {}) {
            Ok(_) => {}
            // should remain valid through these errors
            Err(Error::KeyAlreadyPresent()) => continue,
            Err(Error::HashAlreadyPresent()) => continue,
            // other errors should not be occurring
            Err(error) => panic!("unexpected error: {:?}", error),
        };
    }

    blob.check_integrity().expect("check integrity");
});
