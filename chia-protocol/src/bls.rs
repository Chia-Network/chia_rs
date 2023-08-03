use crate::bytes::{Bytes48, Bytes96};
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pyclass(frozen), derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct G1Element(Bytes48);

#[cfg(feature = "py-bindings")]
impl ToJsonDict for G1Element {
    fn to_json_dict(&self, py: Python) -> pyo3::PyResult<PyObject> {
        self.0.to_json_dict(py)
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for G1Element {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        Ok(Self(Bytes48::from_json_dict(o)?))
    }
}
#[cfg_attr(feature = "py-bindings", pyclass(frozen), derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct G2Element(Bytes96);

#[cfg(feature = "py-bindings")]
impl ToJsonDict for G2Element {
    fn to_json_dict(&self, py: Python) -> pyo3::PyResult<PyObject> {
        self.0.to_json_dict(py)
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for G2Element {
    fn from_json_dict(o: &PyAny) -> PyResult<Self> {
        Ok(Self(Bytes96::from_json_dict(o)?))
    }
}
