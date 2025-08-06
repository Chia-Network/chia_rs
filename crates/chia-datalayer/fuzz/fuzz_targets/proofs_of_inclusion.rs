#![no_main]

use libfuzzer_sys::{
    arbitrary::{Arbitrary, Unstructured},
    fuzz_target,
};

use chia_datalayer::{Error, Hash, InsertLocation, KeyId, MerkleBlob, ValueId};

fuzz_target!(|data: &[u8]| {
    let mut blob = MerkleBlob::new(Vec::new()).unwrap();
    blob.check_integrity_on_drop = false;

    let mut keys: Vec<KeyId> = Vec::new();

    let mut unstructured = Unstructured::new(data);
    while !unstructured.is_empty() {
        let key = KeyId::arbitrary(&mut unstructured).unwrap();
        let value = ValueId::arbitrary(&mut unstructured).unwrap();
        let hash = Hash::arbitrary(&mut unstructured).unwrap();

        match blob.insert(key, value, &hash, InsertLocation::Auto {}) {
            Ok(_) => {
                keys.push(key);
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
