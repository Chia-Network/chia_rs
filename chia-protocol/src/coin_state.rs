use crate::chia_error;
use crate::coin::Coin;
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

#[cfg_attr(feature = "py-bindings", pyclass, derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct CoinState {
    #[cfg_attr(features = "py-bindings", pyo3(get))]
    coin: Coin,
    #[cfg_attr(features = "py-bindings", pyo3(get))]
    spent_height: Option<u32>,
    #[cfg_attr(features = "py-bindings", pyo3(get))]
    created_height: Option<u32>,
}
