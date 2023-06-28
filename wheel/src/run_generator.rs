use chia_protocol::from_json_dict::FromJsonDict;
use chia_protocol::to_json_dict::ToJsonDict;

use chia::allocator::make_allocator;
use chia::gen::conditions::{Spend, SpendBundleConditions};
use chia::gen::run_block_generator::run_block_generator as native_run_block_generator;
use chia::gen::validation_error::ValidationErr;
use chia_protocol::bytes::{Bytes, Bytes32, Bytes48};

use clvmr::cost::Cost;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::PyList;

use chia_protocol::chia_error;
use chia_protocol::streamable::Streamable;
use chia_py_streamable_macro::PyStreamable;
use chia_streamable_macro::Streamable;

#[pyclass(name = "Spend", get_all, frozen)]
#[derive(Streamable, PyStreamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct PySpend {
    pub coin_id: Bytes32,
    pub puzzle_hash: Bytes32,
    pub height_relative: Option<u32>,
    pub seconds_relative: Option<u64>,
    pub before_height_relative: Option<u32>,
    pub before_seconds_relative: Option<u64>,
    pub birth_height: Option<u32>,
    pub birth_seconds: Option<u64>,
    pub create_coin: Vec<(Bytes32, u64, Option<Bytes>)>,
    pub agg_sig_me: Vec<(Bytes48, Bytes)>,
    pub flags: u32,
}

#[pyclass(name = "SpendBundleConditions", get_all, frozen)]
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

fn convert_spend(spend: Spend) -> PySpend {
    let mut create_coin =
        Vec::<(Bytes32, u64, Option<Bytes>)>::with_capacity(spend.create_coin.len());
    for c in spend.create_coin {
        create_coin.push((c.puzzle_hash, c.amount, c.hint));
    }
    create_coin.sort();

    PySpend {
        coin_id: *spend.coin_id,
        puzzle_hash: spend.puzzle_hash,
        height_relative: spend.height_relative,
        seconds_relative: spend.seconds_relative,
        before_height_relative: spend.before_height_relative,
        before_seconds_relative: spend.before_seconds_relative,
        birth_height: spend.birth_height,
        birth_seconds: spend.birth_seconds,
        create_coin,
        agg_sig_me: spend.agg_sig_me,
        flags: spend.flags,
    }
}

pub fn convert_spend_bundle_conds(sb: SpendBundleConditions) -> PySpendBundleConditions {
    let mut spends = Vec::<PySpend>::with_capacity(sb.spends.len());
    for s in sb.spends {
        spends.push(convert_spend(s));
    }

    PySpendBundleConditions {
        spends,
        reserve_fee: sb.reserve_fee,
        height_absolute: sb.height_absolute,
        seconds_absolute: sb.seconds_absolute,
        before_height_absolute: sb.before_height_absolute,
        before_seconds_absolute: sb.before_seconds_absolute,
        agg_sig_unsafe: sb.agg_sig_unsafe.clone(),
        cost: sb.cost,
        removal_amount: sb.removal_amount,
        addition_amount: sb.addition_amount,
    }
}

#[pyfunction]
pub fn run_block_generator(
    _py: Python,
    program: PyBuffer<u8>,
    block_refs: &PyList,
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<PySpendBundleConditions>)> {
    let mut allocator = make_allocator(flags);

    let mut refs = Vec::<&[u8]>::new();
    for g in block_refs {
        let buf = g.extract::<PyBuffer<u8>>()?;

        if !buf.is_c_contiguous() {
            panic!("block_refs buffers must be contiguous");
        }
        let slice =
            unsafe { std::slice::from_raw_parts(buf.buf_ptr() as *const u8, buf.len_bytes()) };
        refs.push(slice);
    }

    if !program.is_c_contiguous() {
        panic!("program buffer must be contiguous");
    }
    let program =
        unsafe { std::slice::from_raw_parts(program.buf_ptr() as *const u8, program.len_bytes()) };

    match native_run_block_generator(&mut allocator, program, &refs, max_cost, flags) {
        Ok(spend_bundle_conds) => {
            // everything was successful
            Ok((None, Some(convert_spend_bundle_conds(spend_bundle_conds))))
        }
        Err(ValidationErr(_, error_code)) => {
            // a validation error occurred
            Ok((Some(error_code.into()), None))
        }
    }
}
