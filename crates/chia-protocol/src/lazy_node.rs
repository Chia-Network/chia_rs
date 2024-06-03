use clvmr::{allocator::NodePtr, allocator::SExp, Allocator};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use std::rc::Rc;

#[pyclass(subclass, unsendable, frozen)]
#[derive(Clone)]
pub struct LazyNode {
    allocator: Rc<Allocator>,
    node: NodePtr,
}

impl ToPyObject for LazyNode {
    fn to_object(&self, py: Python<'_>) -> PyObject {
        Bound::new(py, self.clone()).unwrap().to_object(py)
    }
}

#[pymethods]
impl LazyNode {
    #[getter(pair)]
    pub fn pair(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        match &self.allocator.sexp(self.node) {
            SExp::Pair(p1, p2) => {
                let r1 = Self::new(self.allocator.clone(), *p1);
                let r2 = Self::new(self.allocator.clone(), *p2);
                let v = PyTuple::new_bound(py, &[r1, r2]);
                Ok(Some(v.into()))
            }
            SExp::Atom => Ok(None),
        }
    }

    #[getter(atom)]
    pub fn atom(&self, py: Python<'_>) -> Option<PyObject> {
        match &self.allocator.sexp(self.node) {
            SExp::Atom => {
                Some(PyBytes::new_bound(py, self.allocator.atom(self.node).as_ref()).into())
            }
            SExp::Pair(..) => None,
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
