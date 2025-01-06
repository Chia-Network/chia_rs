use clvmr::{allocator::NodePtr, allocator::SExp, Allocator};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::rc::Rc;

#[pyclass(subclass, unsendable, frozen)]
#[derive(Clone)]
pub struct LazyNode {
    allocator: Rc<Allocator>,
    node: NodePtr,
}

#[pymethods]
impl LazyNode {
    #[getter(pair)]
    pub fn pair(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        match &self.allocator.sexp(self.node) {
            SExp::Pair(p1, p2) => {
                let r1 = Self::new(self.allocator.clone(), *p1);
                let r2 = Self::new(self.allocator.clone(), *p2);
                let v = (r1, r2).into_pyobject(py)?;
                Ok(Some(v.into()))
            }
            SExp::Atom => Ok(None),
        }
    }

    #[getter(atom)]
    pub fn atom(&self, py: Python<'_>) -> Option<PyObject> {
        match &self.allocator.sexp(self.node) {
            SExp::Atom => Some(PyBytes::new(py, self.allocator.atom(self.node).as_ref()).into()),
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
