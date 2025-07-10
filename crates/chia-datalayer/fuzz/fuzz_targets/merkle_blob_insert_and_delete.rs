#![no_main]

use libfuzzer_sys::{
    arbitrary::{Arbitrary, Unstructured},
    fuzz_target,
};

use chia_datalayer::{Error, Hash, InsertLocation, KeyId, MerkleBlob, ValueId};

fuzz_target!(|data: &[u8]| {
    let mut blob = MerkleBlob::new(Vec::new()).unwrap();
    blob.check_integrity_on_drop = false;

    let mut unstructured = Unstructured::new(data);
    while !unstructured.is_empty() {
        if unstructured.ratio(8, 10).unwrap() {
            let key = KeyId::arbitrary(&mut unstructured).unwrap();
            let value = ValueId::arbitrary(&mut unstructured).unwrap();
            let hash = Hash::arbitrary(&mut unstructured).unwrap();

            match blob.insert(key, value, &hash, InsertLocation::Auto {}) {
                Ok(_) => {}
                // should remain valid through these errors
                Err(Error::KeyAlreadyPresent()) => continue,
                Err(Error::HashAlreadyPresent()) => continue,
                // other errors should not be occurring
                Err(error) => panic!("unexpected error while inserting: {:?}", error),
            };
        } else {
            let key = if unstructured.ratio(1, 10).unwrap() {
                KeyId::arbitrary(&mut unstructured).unwrap()
            } else {
                let keys_values = blob.get_keys_values().unwrap();
                let keys: Vec<&KeyId> = keys_values.keys().collect();
                let index = match unstructured.choose_index(keys.len()) {
                    Ok(index) => index,
                    Err(_) => continue,
                };
                **keys.get(index).unwrap()
            };
            match blob.delete(key) {
                Ok(_) => {}
                // should remain valid through these errors
                Err(Error::UnknownKey(_)) => continue,
                // other errors should not be occurring
                Err(error) => panic!("unexpected error while deleting: {:?}", error),
            }
        }
    }

    blob.check_integrity().unwrap();
});
