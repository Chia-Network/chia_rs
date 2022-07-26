use crate::from_json_dict::FromJsonDict;
use crate::to_json_dict::ToJsonDict;
use chia::chia_error;
use chia::streamable::Streamable;
use chia_streamable_macro::Streamable;
use py_streamable::PyStreamable;

use crate::coin_state::CoinState;
use chia::bytes::Bytes32;
use pyo3::prelude::*;

#[pyclass(unsendable)]
#[derive(Streamable, PyStreamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct RespondToPhUpdates {
    #[pyo3(get)]
    puzzle_hashes: Vec<Bytes32>,
    #[pyo3(get)]
    min_height: u32,
    #[pyo3(get)]
    coin_states: Vec<CoinState>,
}
