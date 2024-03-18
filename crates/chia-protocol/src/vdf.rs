use chia_streamable_macro::streamable;

use crate::ClassgroupElement;
use crate::{Bytes, Bytes32};

#[streamable]
pub struct VDFInfo {
    challenge: Bytes32,
    number_of_iterations: u64,
    output: ClassgroupElement,
}

#[streamable]
pub struct VDFProof {
    witness_type: u8,
    witness: Bytes,
    normalized_to_identity: bool,
}
