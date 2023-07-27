use crate::bytes::{Bytes48, Bytes96};
use chia_streamable_macro::Streamable;

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pyclass(frozen), derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct G1Element(Bytes48);

#[cfg_attr(feature = "py-bindings", pyclass(frozen), derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct G2Element(Bytes96);
