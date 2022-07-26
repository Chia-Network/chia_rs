use crate::coin::Coin;
use crate::from_json_dict::FromJsonDict;
use crate::to_json_dict::ToJsonDict;
use chia::chia_error;
use chia::streamable::Streamable;
use chia_streamable_macro::Streamable;
use py_streamable::PyStreamable;

use pyo3::prelude::*;

#[pyclass(unsendable)]
#[derive(Streamable, PyStreamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct CoinState {
    #[pyo3(get)]
    coin: Coin,
    #[pyo3(get)]
    spent_height: Option<u32>,
    #[pyo3(get)]
    created_height: Option<u32>,
}
