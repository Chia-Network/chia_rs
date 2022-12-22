use super::adapt_response::eval_err_to_pyresult;
use chia_protocol::from_json_dict::FromJsonDict;
use chia_protocol::to_json_dict::ToJsonDict;

use chia::gen::conditions::{parse_spends, Spend, SpendBundleConditions};
use chia::gen::validation_error::{ErrorCode, ValidationErr};
use chia_protocol::bytes::{Bytes, Bytes32, Bytes48};

use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::chia_dialect::LIMIT_HEAP;
use clvmr::cost::Cost;
use clvmr::reduction::{EvalErr, Reduction};
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes;

use pyo3::prelude::*;

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
    // Unsafe Agg Sig conditions (i.e. not tied to the spend generating it)
    pub agg_sig_unsafe: Vec<(Bytes48, Bytes)>,
    pub cost: u64,
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
        create_coin,
        agg_sig_me: agg_sigs,
        flags: spend.flags,
    }
}

fn convert_spend_bundle_conds(a: &Allocator, sb: SpendBundleConditions) -> PySpendBundleConditions {
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
        agg_sig_unsafe: agg_sigs,
        cost: sb.cost,
    }
}

// returns the cost of running the CLVM program along with conditions and the list of
// spends
#[pyfunction]
#[allow(clippy::borrow_deref_ref)]
pub fn run_generator(
    py: Python,
    program: &[u8],
    args: &[u8],
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<PySpendBundleConditions>)> {
    let mut allocator = if flags & LIMIT_HEAP != 0 {
        Allocator::new_limited(500000000, 62500000, 62500000)
    } else {
        Allocator::new()
    };
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
        }
        Ok((error_code, _)) => {
            // a validation error occurred
            Ok((error_code.map(|x| x.into()), None))
        }
        Err(eval_err) => eval_err_to_pyresult(py, eval_err, allocator),
    }
}
