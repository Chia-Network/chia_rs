use super::adapt_response::eval_err_to_pyresult;
use chia::allocator::make_allocator;
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Response;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, serialized_length_from_bytes};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use std::rc::Rc;

#[pyclass(subclass, unsendable)]
#[derive(Clone)]
pub struct LazyNode {
    allocator: Rc<Allocator>,
    node: NodePtr,
}

impl ToPyObject for LazyNode {
    fn to_object(&self, py: Python<'_>) -> PyObject {
        let node: &PyCell<LazyNode> = PyCell::new(py, self.clone()).unwrap();
        let pa: &PyAny = node;
        pa.to_object(py)
    }
}

#[pymethods]
impl LazyNode {
    #[getter(pair)]
    pub fn pair(&self, py: Python) -> PyResult<Option<PyObject>> {
        match &self.allocator.sexp(self.node) {
            SExp::Pair(p1, p2) => {
                let r1 = Self::new(self.allocator.clone(), *p1);
                let r2 = Self::new(self.allocator.clone(), *p2);
                let v: &PyTuple = PyTuple::new(py, &[r1, r2]);
                Ok(Some(v.into()))
            }
            _ => Ok(None),
        }
    }

    #[getter(atom)]
    pub fn atom(&self, py: Python) -> Option<PyObject> {
        match &self.allocator.sexp(self.node) {
            SExp::Atom(atom) => Some(PyBytes::new(py, self.allocator.buf(atom)).into()),
            _ => None,
        }
    }
}

impl LazyNode {
    pub const fn new(a: Rc<Allocator>, n: NodePtr) -> Self {
        Self {
            allocator: a,
            node: n,
        }
    }
}

#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn serialized_length(program: &[u8]) -> PyResult<u64> {
    Ok(serialized_length_from_bytes(program)?)
}

#[allow(clippy::borrow_deref_ref)]
#[pyfunction]
pub fn run_chia_program(
    py: Python,
    program: &[u8],
    args: &[u8],
    max_cost: Cost,
    flags: u32,
) -> PyResult<(Cost, LazyNode)> {
    let mut allocator = make_allocator(flags);

    let r: Response = (|| -> PyResult<Response> {
        let program = node_from_bytes(&mut allocator, program)?;
        let args = node_from_bytes(&mut allocator, args)?;
        let dialect = ChiaDialect::new(flags);

        Ok(py
            .allow_threads(|| run_program(&mut allocator, &dialect, program, args, max_cost)))
    })()?;
    match r {
        Ok(reduction) => {
            let val = LazyNode::new(Rc::new(allocator), reduction.1);
            Ok((reduction.0, val))
        }
        Err(eval_err) => eval_err_to_pyresult(py, eval_err, allocator),
    }
}
