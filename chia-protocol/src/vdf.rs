use crate::streamable_struct;
use chia_streamable_macro::Streamable;

use crate::ClassgroupElement;
use crate::{Bytes, Bytes32};

streamable_struct!(VDFInfo {
    challenge: Bytes32,
    number_of_iterations: u64,
    output: ClassgroupElement,
});

streamable_struct!(VDFProof {
    witness_type: u8,
    witness: Bytes,
    normalized_to_identity: bool,
});
