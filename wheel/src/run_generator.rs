use chia_consensus::allocator::make_allocator;
use chia_consensus::gen::conditions::{EmptyVisitor, MempoolVisitor, Spend, SpendBundleConditions};
use chia_consensus::gen::flags::ANALYZE_SPENDS;
use chia_consensus::gen::owned_conditions::{OwnedSpend, OwnedSpendBundleConditions};
use chia_consensus::gen::run_block_generator::run_block_generator as native_run_block_generator;
use chia_consensus::gen::run_block_generator::run_block_generator2 as native_run_block_generator2;
use chia_consensus::gen::validation_error::ValidationErr;
use chia_protocol::bytes::{Bytes, Bytes32, Bytes48};

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::cost::Cost;

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;
use pyo3::types::PyList;

fn convert_agg_sigs(a: &Allocator, agg_sigs: &[(NodePtr, NodePtr)]) -> Vec<(Bytes48, Bytes)> {
    let mut ret = Vec::<(Bytes48, Bytes)>::new();
    for (pk, msg) in agg_sigs {
        ret.push((
            a.atom(*pk).as_ref().try_into().unwrap(),
            a.atom(*msg).as_ref().into(),
        ));
    }
    ret
}

fn convert_spend(a: &Allocator, spend: Spend) -> OwnedSpend {
    let mut create_coin =
        Vec::<(Bytes32, u64, Option<Bytes>)>::with_capacity(spend.create_coin.len());
    for c in spend.create_coin {
        create_coin.push((
            c.puzzle_hash,
            c.amount,
            if c.hint != a.nil() {
                Some(a.atom(c.hint).as_ref().into())
            } else {
                None
            },
        ));
    }

    OwnedSpend {
        coin_id: *spend.coin_id,
        parent_id: a.atom(spend.parent_id).as_ref().try_into().unwrap(),
        puzzle_hash: a.atom(spend.puzzle_hash).as_ref().try_into().unwrap(),
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
) -> OwnedSpendBundleConditions {
    let mut spends = Vec::<OwnedSpend>::new();
    for s in sb.spends {
        spends.push(convert_spend(a, s));
    }

    let mut agg_sigs = Vec::<(Bytes48, Bytes)>::with_capacity(sb.agg_sig_unsafe.len());
    for (pk, msg) in sb.agg_sig_unsafe {
        agg_sigs.push((
            a.atom(pk).as_ref().try_into().unwrap(),
            a.atom(msg).as_ref().into(),
        ));
    }

    OwnedSpendBundleConditions {
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
) -> PyResult<(Option<u32>, Option<OwnedSpendBundleConditions>)> {
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

    let run_block = if (flags & ANALYZE_SPENDS) == 0 {
        native_run_block_generator::<_, EmptyVisitor>
    } else {
        native_run_block_generator::<_, MempoolVisitor>
    };

    Ok(
        match run_block(&mut allocator, program, &refs, max_cost, flags) {
            Ok(spend_bundle_conds) => {
                // everything was successful
                (
                    None,
                    Some(convert_spend_bundle_conds(&allocator, spend_bundle_conds)),
                )
            }
            Err(ValidationErr(_, error_code)) => {
                // a validation error occurred
                (Some(error_code.into()), None)
            }
        },
    )
}

#[pyfunction]
pub fn run_block_generator2(
    _py: Python,
    program: PyBuffer<u8>,
    block_refs: &PyList,
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Option<u32>, Option<OwnedSpendBundleConditions>)> {
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

    let run_block = if (flags & ANALYZE_SPENDS) == 0 {
        native_run_block_generator2::<_, EmptyVisitor>
    } else {
        native_run_block_generator2::<_, MempoolVisitor>
    };

    Ok(
        match run_block(&mut allocator, program, &refs, max_cost, flags) {
            Ok(spend_bundle_conds) => {
                // everything was successful
                (
                    None,
                    Some(convert_spend_bundle_conds(&allocator, spend_bundle_conds)),
                )
            }
            Err(ValidationErr(_, error_code)) => {
                // a validation error occurred
                (Some(error_code.into()), None)
            }
        },
    )
}
