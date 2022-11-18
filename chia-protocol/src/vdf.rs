use crate::streamable_struct;
use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::ClassgroupElement;
use crate::Streamable;
use crate::{Bytes, Bytes32};

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

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
