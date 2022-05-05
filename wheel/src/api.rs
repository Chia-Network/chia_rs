use crate::run_generator::{PySpend, PySpendBundleConditions, __pyo3_get_function_run_generator};
use chia::gen::flags::COND_ARGS_NIL;
use chia::gen::flags::COND_CANON_INTS;
use chia::gen::flags::NO_UNKNOWN_CONDS;
use chia::gen::flags::STRICT_ARGS_COUNT;
use chia::merkle_set::compute_merkle_set_root as compute_merkle_root_impl;
use std::convert::TryInto;
//use chia::streamable::coin::Coin;
//use chia::streamable::fullblock::Fullblock;
use clvmr::chia_dialect::NO_NEG_DIV;
use clvmr::chia_dialect::NO_UNKNOWN_OPS;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyModule;
use pyo3::{wrap_pyfunction, PyResult, Python};

use crate::run_program::{
    __pyo3_get_function_run_chia_program, __pyo3_get_function_serialized_length,
};

pub const MEMPOOL_MODE: u32 = NO_NEG_DIV
    | COND_CANON_INTS
    | NO_UNKNOWN_CONDS
    | NO_UNKNOWN_OPS
    | COND_ARGS_NIL
    | STRICT_ARGS_COUNT;

#[pyfunction]
pub fn compute_merkle_set_root<'p>(
    py: Python<'p>,
    values: Vec<&'p PyBytes>,
) -> PyResult<&'p PyBytes> {
    let mut buffer = Vec::<[u8; 32]>::with_capacity(values.len());
    for b in values {
        buffer.push(b.as_bytes().try_into()?);
    }
    Ok(PyBytes::new(py, &compute_merkle_root_impl(&mut buffer)))
}

#[pymodule]
pub fn chia_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_generator, m)?)?;
    m.add_class::<PySpendBundleConditions>()?;
    m.add_class::<PySpend>()?;
    m.add("COND_CANON_INTS", COND_CANON_INTS)?;
    m.add("COND_ARGS_NIL", COND_ARGS_NIL)?;
    m.add("NO_UNKNOWN_CONDS", NO_UNKNOWN_CONDS)?;
    m.add("STRICT_ARGS_COUNT", STRICT_ARGS_COUNT)?;
    m.add("MEMPOOL_MODE", MEMPOOL_MODE)?;
    //m.add_class::<Coin>()?;
    //m.add_class::<Fullblock>()?;

    // facilities from clvm_rs

    m.add_function(wrap_pyfunction!(run_chia_program, m)?)?;
    m.add("NO_NEG_DIV", NO_NEG_DIV)?;
    m.add("NO_UNKNOWN_OPS", NO_UNKNOWN_OPS)?;

    m.add_function(wrap_pyfunction!(serialized_length, m)?)?;
    m.add_function(wrap_pyfunction!(compute_merkle_set_root, m)?)?;

    Ok(())
}
