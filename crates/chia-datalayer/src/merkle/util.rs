use crate::Hash;
use chia_protocol::Bytes32;
use chia_sha2::Sha256;
use num_traits::ToBytes;

pub fn sha256_num<T: ToBytes>(input: &T) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input.to_be_bytes());

    Hash(Bytes32::new(hasher.finalize()))
}

pub fn sha256_bytes(input: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(input);

    Hash(Bytes32::new(hasher.finalize()))
}
