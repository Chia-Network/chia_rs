use crate::run_generator::{PySpend, PySpendBundleConditions, __pyo3_get_function_run_generator};
use chia::gen::flags::COND_ARGS_NIL;
use chia::gen::flags::COND_CANON_INTS;
use chia::gen::flags::NO_UNKNOWN_CONDS;
//use chia::streamable::coin::Coin;
//use chia::streamable::fullblock::Fullblock;
use clvmr::chia_dialect::NO_NEG_DIV;
use clvmr::chia_dialect::NO_UNKNOWN_OPS;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::{wrap_pyfunction, PyResult, Python};

pub const MEMPOOL_MODE: u32 =
    NO_NEG_DIV | COND_CANON_INTS | NO_UNKNOWN_CONDS | NO_UNKNOWN_OPS | COND_ARGS_NIL;

#[pymodule]
pub fn chia_rs(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(a_test_function, m)?)?;
    m.add_function(wrap_pyfunction!(run_generator, m)?)?;
    m.add_class::<PySpendBundleConditions>()?;
    m.add_class::<PySpend>()?;
    m.add("NO_NEG_DIV", NO_NEG_DIV)?;
    m.add("COND_CANON_INTS", COND_CANON_INTS)?;
    m.add("COND_ARGS_NIL", COND_ARGS_NIL)?;
    m.add("NO_UNKNOWN_CONDS", NO_UNKNOWN_CONDS)?;
    m.add("NO_UNKNOWN_OPS", NO_UNKNOWN_OPS)?;
    m.add("MEMPOOL_MODE", MEMPOOL_MODE)?;
    //m.add_class::<Coin>()?;
    //m.add_class::<Fullblock>()?;

    Ok(())
}

#[pyfunction]
pub fn a_test_function() -> u128 {
    500
}
