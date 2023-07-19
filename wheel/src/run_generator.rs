use chia_protocol::from_json_dict::FromJsonDict;
use chia_protocol::to_json_dict::ToJsonDict;

use chia::allocator::make_allocator;
use chia::gen::conditions::{Spend, SpendBundleConditions};
use chia::gen::run_block_generator::run_block_generator as native_run_block_generator;
use chia::gen::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia::gen::validation_error::ValidationErr;
use chia_protocol::bytes::{Bytes, Bytes32, Bytes48};

use clvmr::allocator::{Allocator, NodePtr};
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

fn convert_agg_sigs(a: &Allocator, agg_sigs: &[(NodePtr, NodePtr)]) -> Vec<(Bytes48, Bytes)> {
    let mut ret = Vec::<(Bytes48, Bytes)>::new();
    for (pk, msg) in agg_sigs {
        ret.push((a.atom(*pk).into(), a.atom(*msg).into()));
    }
    ret
}

fn convert_spend(a: &Allocator, spend: Spend) -> PySpend {
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
        parent_id: a.atom(spend.parent_id).into(),
        puzzle_hash: a.atom(spend.puzzle_hash).into(),
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
    }
}

pub fn convert_spend_bundle_conds(
    a: &Allocator,
    sb: SpendBundleConditions,
) -> PySpendBundleConditions {
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
            Ok((
                None,
                Some(convert_spend_bundle_conds(&allocator, spend_bundle_conds)),
            ))
        }
        Err(ValidationErr(_, error_code)) => {
            // a validation error occurred
            Ok((Some(error_code.into()), None))
        }
    }
}

#[pyfunction]
pub fn run_block_generator2(
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

    match native_run_block_generator2(&mut allocator, program, &refs, max_cost, flags) {
        Ok(spend_bundle_conds) => {
            // everything was successful
            Ok((
                None,
                Some(convert_spend_bundle_conds(&allocator, spend_bundle_conds)),
            ))
        }
        Err(ValidationErr(_, error_code)) => {
            // a validation error occurred
            Ok((Some(error_code.into()), None))
        }
    }
}
