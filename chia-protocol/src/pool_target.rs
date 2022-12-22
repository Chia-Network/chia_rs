use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::streamable_struct;
use crate::Bytes32;
use crate::Streamable;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct!(PoolTarget {
    puzzle_hash: Bytes32,
    max_height: u32, // A max height of 0 means it is valid forever
});
