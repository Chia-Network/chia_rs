use clvmr::allocator::Allocator;
use clvmr::node::Node;
use clvmr::reduction::EvalErr;
use clvmr::serialize::node_to_bytes;

use pyo3::prelude::*;
use pyo3::types::PyDict;

pub fn eval_err_to_pyresult<T>(py: Python, eval_err: EvalErr, allocator: Allocator) -> PyResult<T> {
    let node = Node::new(&allocator, eval_err.0);
    let blob = node_to_bytes(&node).unwrap();
    let msg = eval_err.1;
    let ctx: &PyDict = PyDict::new(py);
    ctx.set_item("msg", msg)?;
    ctx.set_item("blob", blob)?;
    Err(py
        .run("raise ValueError(msg, bytes(blob).hex())", None, Some(ctx))
        .unwrap_err())
}
