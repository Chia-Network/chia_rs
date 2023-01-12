use clvmr::allocator::Allocator;
use clvmr::node::Node;
use clvmr::reduction::EvalErr;
use clvmr::serde::node_to_bytes;

use pyo3::prelude::*;
use pyo3::types::PyDict;

pub fn eval_err_to_pyresult<T>(py: Python, eval_err: EvalErr, allocator: Allocator) -> PyResult<T> {
    let node = Node::new(&allocator, eval_err.0);
    let ctx: &PyDict = PyDict::new(py);
    let msg = eval_err.1;
    ctx.set_item("msg", msg)?;
    if let Ok(blob) = node_to_bytes(&node) {
        ctx.set_item("blob", blob)?;
    }
    Err(py
        .run("raise ValueError(msg, bytes(blob).hex())", None, Some(ctx))
        .unwrap_err())
}
