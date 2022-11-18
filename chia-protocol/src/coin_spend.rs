use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::coin::Coin;
use crate::program::Program;
use crate::streamable::Streamable;
use crate::streamable_struct;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct!(CoinSpend {
    coin: Coin,
    puzzle_reveal: Program,
    solution: Program,
});
