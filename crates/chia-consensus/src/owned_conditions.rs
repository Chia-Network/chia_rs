use chia_bls::PublicKey;
use chia_protocol::{Bytes, Bytes32};
use chia_streamable_macro::Streamable;
use clvmr::{Allocator, NodePtr};

use super::conditions::{SpendBundleConditions, SpendConditions};

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};

#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyNotImplementedError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;

#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(name = "SpendConditions", get_all, frozen),
    derive(PyJsonDict, PyStreamable)
)]
pub struct OwnedSpendConditions {
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
    pub agg_sig_me: Vec<(PublicKey, Bytes)>,
    pub agg_sig_parent: Vec<(PublicKey, Bytes)>,
    pub agg_sig_puzzle: Vec<(PublicKey, Bytes)>,
    pub agg_sig_amount: Vec<(PublicKey, Bytes)>,
    pub agg_sig_puzzle_amount: Vec<(PublicKey, Bytes)>,
    pub agg_sig_parent_amount: Vec<(PublicKey, Bytes)>,
    pub agg_sig_parent_puzzle: Vec<(PublicKey, Bytes)>,
    pub flags: u32,
    /// per-spend execution and condition cost
    pub execution_cost: u64,
    pub condition_cost: u64,
}

#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(name = "SpendBundleConditions", get_all, frozen),
    derive(PyJsonDict, PyStreamable)
)]
pub struct OwnedSpendBundleConditions {
    pub spends: Vec<OwnedSpendConditions>,
    pub reserve_fee: u64,
    /// the highest height/time conditions (i.e. most strict)
    pub height_absolute: u32,
    pub seconds_absolute: u64,
    /// when set, this is the lowest (i.e. most restrictive) of all
    /// ASSERT_BEFORE_HEIGHT_ABSOLUTE conditions
    pub before_height_absolute: Option<u32>,
    /// ASSERT_BEFORE_SECONDS_ABSOLUTE conditions
    pub before_seconds_absolute: Option<u64>,
    /// Unsafe Agg Sig conditions (i.e. not tied to the spend generating it)
    pub agg_sig_unsafe: Vec<(PublicKey, Bytes)>,
    pub cost: u64,
    /// the sum of all values of all spent coins
    pub removal_amount: u128,
    /// the sum of all amounts of CREATE_COIN conditions
    pub addition_amount: u128,
    /// set if the aggregate signature of the block/spend bundle was
    /// successfully validated
    pub validated_signature: bool,
    pub execution_cost: u64,
    pub condition_cost: u64,
}

impl OwnedSpendConditions {
    pub fn from(a: &Allocator, spend: SpendConditions) -> Self {
        let mut create_coin =
            Vec::<(Bytes32, u64, Option<Bytes>)>::with_capacity(spend.create_coin.len());
        for c in spend.create_coin {
            create_coin.push((
                c.puzzle_hash,
                c.amount,
                if c.hint == a.nil() {
                    None
                } else {
                    Some(a.atom(c.hint).as_ref().into())
                },
            ));
        }

        Self {
            coin_id: *spend.coin_id,
            parent_id: a
                .atom(spend.parent_id)
                .as_ref()
                .try_into()
                .expect("OwnedSpend internal error (parent_id)"),
            puzzle_hash: a
                .atom(spend.puzzle_hash)
                .as_ref()
                .try_into()
                .expect("OwnedSpend internal error (puzzle_hash)"),
            coin_amount: spend.coin_amount,
            height_relative: spend.height_relative,
            seconds_relative: spend.seconds_relative,
            before_height_relative: spend.before_height_relative,
            before_seconds_relative: spend.before_seconds_relative,
            birth_height: spend.birth_height,
            birth_seconds: spend.birth_seconds,
            create_coin,
            agg_sig_me: convert_agg_sigs(a, &spend.agg_sig_me),
            agg_sig_parent: convert_agg_sigs(a, &spend.agg_sig_parent),
            agg_sig_puzzle: convert_agg_sigs(a, &spend.agg_sig_puzzle),
            agg_sig_amount: convert_agg_sigs(a, &spend.agg_sig_amount),
            agg_sig_puzzle_amount: convert_agg_sigs(a, &spend.agg_sig_puzzle_amount),
            agg_sig_parent_amount: convert_agg_sigs(a, &spend.agg_sig_parent_amount),
            agg_sig_parent_puzzle: convert_agg_sigs(a, &spend.agg_sig_parent_puzzle),
            flags: spend.flags,
            execution_cost: spend.execution_cost,
            condition_cost: spend.condition_cost,
        }
    }
}

impl OwnedSpendBundleConditions {
    pub fn from(a: &Allocator, sb: SpendBundleConditions) -> Self {
        let mut spends = Vec::<OwnedSpendConditions>::new();
        for s in sb.spends {
            spends.push(OwnedSpendConditions::from(a, s));
        }

        let mut agg_sigs = Vec::<(PublicKey, Bytes)>::with_capacity(sb.agg_sig_unsafe.len());
        for (pk, msg) in sb.agg_sig_unsafe {
            agg_sigs.push((pk, a.atom(msg).as_ref().into()));
        }

        Self {
            spends,
            reserve_fee: sb.reserve_fee,
            height_absolute: sb.height_absolute,
            seconds_absolute: sb.seconds_absolute,
            before_height_absolute: sb.before_height_absolute,
            before_seconds_absolute: sb.before_seconds_absolute,
            agg_sig_unsafe: agg_sigs,
            cost: sb.cost,
            removal_amount: sb.removal_amount,
            addition_amount: sb.addition_amount,
            validated_signature: sb.validated_signature,
            execution_cost: sb.execution_cost,
            condition_cost: sb.condition_cost,
        }
    }
}

fn convert_agg_sigs(a: &Allocator, agg_sigs: &[(PublicKey, NodePtr)]) -> Vec<(PublicKey, Bytes)> {
    let mut ret = Vec::<(PublicKey, Bytes)>::new();
    for (pk, msg) in agg_sigs {
        ret.push((*pk, a.atom(*msg).as_ref().into()));
    }
    ret
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl OwnedSpendConditions {
    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "OwnedSpendConditions does not support from_parent().",
        ))
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl OwnedSpendBundleConditions {
    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _instance: &Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "OwnedSpendBundleConditions does not support from_parent().",
        ))
    }
}
