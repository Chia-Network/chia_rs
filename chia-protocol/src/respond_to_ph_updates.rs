use crate::bytes::Bytes32;
use crate::chia_error;
use crate::coin_state::CoinState;
use crate::streamable::Streamable;
use crate::streamable_struct;
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct! (RespondToPhUpdates {
    puzzle_hashes: Vec<Bytes32>,
    min_height: u32,
    coin_states: Vec<CoinState>,
});
