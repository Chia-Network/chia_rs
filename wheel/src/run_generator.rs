use super::adapt_response::eval_err_to_pyresult;
use chia_protocol::from_json_dict::FromJsonDict;
use chia_protocol::to_json_dict::ToJsonDict;

use chia::gen::conditions::{parse_spends, Spend, SpendBundleConditions};
use chia::gen::validation_error::{ErrorCode, ValidationErr};
use chia::gen::run_block_generator::run_block_generator as native_run_block_generator;
use chia_protocol::bytes::{Bytes, Bytes32, Bytes48};
use chia::allocator::make_allocator;

use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::{EvalErr, Reduction};
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes;

use pyo3::prelude::*;
use pyo3::types::PyList;

use chia_protocol::chia_error;
use chia_protocol::streamable::Streamable;
use chia_py_streamable_macro::PyStreamable;
use chia_streamable_macro::Streamable;

#[pyclass(name = "Spend")]
#[derive(Streamable, PyStreamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct PySpend {
    pub coin_id: Bytes32,
    pub puzzle_hash: Bytes32,
    pub height_relative: Option<u32>,
    pub seconds_relative: u64,
    pub before_height_relative: Option<u32>,
    pub before_seconds_relative: Option<u64>,
    pub birth_height: Option<u32>,
    pub birth_seconds: Option<u64>,
    pub create_coin: Vec<(Bytes32, u64, Option<Bytes>)>,
    pub agg_sig_me: Vec<(Bytes48, Bytes)>,
    pub flags: u32,
}

#[pyclass(name = "SpendBundleConditions")]
#[derive(Streamable, PyStreamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct PySpendBundleConditions {
    pub spends: Vec<PySpend>,
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

fn convert_spend(a: &Allocator, spend: Spend) -> PySpend {
    let mut agg_sigs = Vec::<(Bytes48, Bytes)>::new();
    for (pk, msg) in spend.agg_sig_me {
        agg_sigs.push((a.atom(pk).into(), a.atom(msg).into()));
    }
    let mut create_coin =
        Vec::<(Bytes32, u64, Option<Bytes>)>::with_capacity(spend.create_coin.len());
    for c in spend.create_coin {
        create_coin.push((
            c.puzzle_hash,
            c.amount,
            if c.hint != a.null() {
                Some(a.atom(c.hint).into())
            } else {
                None
            },
        ));
    }

    PySpend {
        coin_id: *spend.coin_id,
        puzzle_hash: a.atom(spend.puzzle_hash).into(),
        height_relative: spend.height_relative,
        seconds_relative: spend.seconds_relative,
        before_height_relative: spend.before_height_relative,
        before_seconds_relative: spend.before_seconds_relative,
        birth_height: spend.birth_height,
        birth_seconds: spend.birth_seconds,
        create_coin,
        agg_sig_me: agg_sigs,
        flags: spend.flags,
    }
}

pub fn convert_spend_bundle_conds(a: &Allocator, sb: SpendBundleConditions) -> PySpendBundleConditions {
    let mut spends = Vec::<PySpend>::new();
    for s in sb.spends {
        spends.push(convert_spend(a, s));
    }

    let mut agg_sigs = Vec::<(Bytes48, Bytes)>::with_capacity(sb.agg_sig_unsafe.len());
    for (pk, msg) in sb.agg_sig_unsafe {
        agg_sigs.push((a.atom(pk).into(), a.atom(msg).into()));
    }

    PySpendBundleConditions {
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
    }
}

// returns the cost of running the CLVM program along with conditions and the list of
// spends
#[pyfunction]
pub fn run_generator(
    py: Python,
    program: &[u8],
    args: &[u8],
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<PySpendBundleConditions>)> {
	let mut allocator = make_allocator(flags);
    let program = node_from_bytes(&mut allocator, program)?;
    let args = node_from_bytes(&mut allocator, args)?;
    let dialect = &ChiaDialect::new(flags);

    let r = py.allow_threads(
        || -> Result<(Option<ErrorCode>, Option<SpendBundleConditions>), EvalErr> {
            let Reduction(cost, node) =
                run_program(&mut allocator, dialect, program, args, max_cost)?;
            // we pass in what's left of max_cost here, to fail early in case the
            // cost of a condition brings us over the cost limit
            match parse_spends(&allocator, node, max_cost - cost, flags) {
                Err(ValidationErr(_, c)) => Ok((Some(c), None)),
                Ok(mut spend_bundle_conds) => {
                    // the cost is only the cost of conditions, add the
                    // cost of running the CLVM program here as well
                    spend_bundle_conds.cost += cost;
                    Ok((None, Some(spend_bundle_conds)))
                }
            }
        },
    );

    match r {
        Ok((None, Some(spend_bundle_conds))) => {
            // everything was successful
            Ok((
                None,
                Some(convert_spend_bundle_conds(&allocator, spend_bundle_conds)),
            ))
        },
        Ok((error_code, _)) => {
            // a validation error occurred
            Ok((error_code.map(|x| x.into()), None))
        }
        Err(eval_err) => eval_err_to_pyresult(py, eval_err, allocator),
    }
}

#[pyfunction]
pub fn run_block_generator(
    _py: Python,
    program: &[u8],
    block_refs: &PyList,
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<PySpendBundleConditions>)> {
	let mut allocator = make_allocator(flags);

    let mut refs = Vec::<Vec<u8>>::new();
    for g in block_refs {
        refs.push(g.extract::<Vec<u8>>()?);
    }

    match native_run_block_generator(&mut allocator, program, &refs, max_cost, flags) {
        Ok(spend_bundle_conds) => {
            // everything was successful
            Ok((
                None,
                Some(convert_spend_bundle_conds(&allocator, spend_bundle_conds)),
            ))
        },
        Err(ValidationErr(_, error_code)) => {
            // a validation error occurred
            Ok((Some(error_code.into()), None))
        }
    }
}
