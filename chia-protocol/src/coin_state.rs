use crate::chia_error;
use crate::coin::Coin;
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

streamable_struct! (CoinState {
    coin: Coin,
    spent_height: Option<u32>,
    created_height: Option<u32>,
});
