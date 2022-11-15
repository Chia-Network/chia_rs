use crate::bytes::Bytes32;
use crate::chia_error;
use crate::coin_state::CoinState;
use crate::streamable::Streamable;
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pyclass(unsendable), derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct RespondToPhUpdates {
    #[cfg_attr(features = "py-bindings", pyo3(get))]
    puzzle_hashes: Vec<Bytes32>,
    #[cfg_attr(features = "py-bindings", pyo3(get))]
    min_height: u32,
    #[cfg_attr(features = "py-bindings", pyo3(get))]
    coin_states: Vec<CoinState>,
}
