#![no_main]

use libfuzzer_sys::{
    arbitrary::{Arbitrary, Unstructured},
    fuzz_target,
};

use chia_datalayer::{
    merkle::dot::open_dot, Error, Hash, InsertLocation, KeyId, MerkleBlob, ValueId,
};

fuzz_target!(|data: &[u8]| {
    let mut blob = MerkleBlob::new(Vec::new()).unwrap();
    println!(" ===== fresh tree");
    blob.check_integrity_on_drop = false;

    let max_count = 1000;
    let value_offset = 2 * max_count;

    let mut unstructured = Unstructured::new(data);
    for raw_key in 0..unstructured.int_in_range(10..=max_count).unwrap() {
        let key = if unstructured.ratio(1, 10).unwrap() {
            KeyId::arbitrary(&mut unstructured).unwrap()
        } else {
            KeyId(raw_key)
        };

        let value = if unstructured.ratio(1, 10).unwrap() {
            ValueId::arbitrary(&mut unstructured).unwrap()
        } else {
            ValueId(raw_key + value_offset)
        };

        let hash = if unstructured.ratio(1, 10).unwrap() {
            Hash::arbitrary(&mut unstructured).unwrap()
        } else {
            use chia_protocol::Bytes32;
            use chia_sha2::Sha256;

            let mut hasher = Sha256::new();
            hasher.update(raw_key.to_be_bytes());

            Hash(Bytes32::new(hasher.finalize()))
        };

        // println!("     attempt ({raw_key:?}): key>{key:?} value>{value:?} hash>{hash:?}");
        let index = match blob.insert(key, value, &hash, InsertLocation::Auto {}) {
            Ok(index) => index,
            Err(Error::KeyAlreadyPresent()) => {
                // println!("key already present: key>{key:?}");
                continue;
            }
            Err(Error::HashAlreadyPresent()) => {
                // println!("hash already present: hash>{hash:?}");
                continue;
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        };
        // println!("inserted: index>{index:?} key>{key:?} value>{value:?} hash>{hash:?}");
        // open_dot(&mut blob.to_dot().unwrap());
    }

    blob.check_integrity().unwrap();
});
