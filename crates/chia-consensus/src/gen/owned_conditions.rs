use chia_protocol::{Bytes, Bytes32, Bytes48};
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};

#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(name = "Spend", get_all, frozen),
    derive(PyJsonDict, PyStreamable)
)]
pub struct OwnedSpend {
    pub coin_id: Bytes32,
    pub parent_id: Bytes32,
    pub puzzle_hash: Bytes32,
    pub coin_amount: u64,
    pub height_relative: Option<u32>,
    pub seconds_relative: Option<u64>,
    pub before_height_relative: Option<u32>,
    pub before_seconds_relative: Option<u64>,
    pub birth_height: Option<u32>,
    pub birth_seconds: Option<u64>,
    pub create_coin: Vec<(Bytes32, u64, Option<Bytes>)>,
    pub agg_sig_me: Vec<(Bytes48, Bytes)>,
    pub agg_sig_parent: Vec<(Bytes48, Bytes)>,
    pub agg_sig_puzzle: Vec<(Bytes48, Bytes)>,
    pub agg_sig_amount: Vec<(Bytes48, Bytes)>,
    pub agg_sig_puzzle_amount: Vec<(Bytes48, Bytes)>,
    pub agg_sig_parent_amount: Vec<(Bytes48, Bytes)>,
    pub agg_sig_parent_puzzle: Vec<(Bytes48, Bytes)>,
    pub flags: u32,
}

#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(name = "SpendBundleConditions", get_all, frozen),
    derive(PyJsonDict, PyStreamable)
)]
pub struct OwnedSpendBundleConditions {
    pub spends: Vec<OwnedSpend>,
    pub reserve_fee: u64,
    // the highest height/time conditions (i.e. most strict)
    pub height_absolute: u32,
    pub seconds_absolute: u64,
    // when set, this is the lowest (i.e. most restrictive) of all
    // ASSERT_BEFORE_HEIGHT_ABSOLUTE conditions
    pub before_height_absolute: Option<u32>,
    // ASSERT_BEFORE_SECONDS_ABSOLUTE conditions
    pub before_seconds_absolute: Option<u64>,
    // Unsafe Agg Sig conditions (i.e. not tied to the spend generating it)
    pub agg_sig_unsafe: Vec<(Bytes48, Bytes)>,
    pub cost: u64,
    // the sum of all values of all spent coins
    pub removal_amount: u128,
    // the sum of all amounts of CREATE_COIN conditions
    pub addition_amount: u128,
}
