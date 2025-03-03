// This is used for the CR Cat and the Clawback spends
use chia_protocol::{Bytes, Bytes32};
use chia_streamable_macro::streamable;

#[streamable]
pub struct VersionedBlob {
    version: u16,
    blob: Bytes,
}

#[streamable]
pub struct ClawbackMetadata {
    timelock: u64,
    sender_puzzle_hash: Bytes32,
    recipient_puzzle_hash: Bytes32,
}
