use crate::merkle::dot::DotLines;
use crate::merkle::util::{sha256_bytes, sha256_num};
use crate::{Hash, InsertLocation, KeyId, MerkleBlob, Side, TreeIndex, ValueId};
use chia_protocol::Bytes32;
use rstest::fixture;

pub const HASH_ZERO: Hash = Hash(Bytes32::new([0; 32]));
pub const HASH_ONE: Hash = Hash(Bytes32::new([1; 32]));
pub const HASH_TWO: Hash = Hash(Bytes32::new([2; 32]));

pub fn open_dot(_lines: &mut DotLines) {
    // crate::merkle::dot::open_dot(_lines);
}

#[fixture]
pub fn small_blob() -> MerkleBlob {
    let mut blob = MerkleBlob::new(vec![]).unwrap();

    blob.insert(
        KeyId(0x0001_0203_0405_0607),
        ValueId(0x1011_1213_1415_1617),
        &sha256_num(&0x1020),
        InsertLocation::Auto {},
    )
    .unwrap();

    blob.insert(
        KeyId(0x2021_2223_2425_2627),
        ValueId(0x3031_3233_3435_3637),
        &sha256_num(&0x2030),
        InsertLocation::Auto {},
    )
    .unwrap();

    blob
}

#[fixture]
pub fn traversal_blob(mut small_blob: MerkleBlob) -> MerkleBlob {
    small_blob
        .insert(
            KeyId(103),
            ValueId(204),
            &sha256_num(&0x1324),
            InsertLocation::Leaf {
                index: TreeIndex(1),
                side: Side::Right,
            },
        )
        .unwrap();
    small_blob
        .insert(
            KeyId(307),
            ValueId(404),
            &sha256_num(&0x9183),
            InsertLocation::Leaf {
                index: TreeIndex(3),
                side: Side::Right,
            },
        )
        .unwrap();

    small_blob.calculate_lazy_hashes().unwrap();
    small_blob
}

pub fn generate_kvid(seed: i32) -> (KeyId, ValueId) {
    let mut kv_ids: Vec<i64> = Vec::new();

    for offset in 0..2 {
        let seed_int = 2i64 * i64::from(seed) + offset;
        let seed_bytes = seed_int.to_be_bytes();
        let hash = sha256_bytes(&seed_bytes);
        let hash_int = i64::from_be_bytes(hash.0[0..8].try_into().unwrap());
        kv_ids.push(hash_int);
    }

    (KeyId(kv_ids[0]), ValueId(kv_ids[1]))
}

pub fn generate_hash(seed: i32) -> Hash {
    let seed_bytes = seed.to_be_bytes();
    sha256_bytes(&seed_bytes)
}
