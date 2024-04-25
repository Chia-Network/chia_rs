use chia_consensus::gen::conditions::{
    parse_conditions, MempoolVisitor, ParseState, Spend, SpendBundleConditions,
};


#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(module = "chia_rs"),
    derive(PyJsonDict, PyStreamable, PyGetters),
    py_uppercase,
    py_pickle
)]
#[streamable]
pub struct NPCResult {
    error: Option<u16>,
    conds: Option<SpendBundleConditions>,
}