use chia_streamable_macro::streamable;

use crate::coin::Coin;
use crate::program::Program;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;

#[streamable]
pub struct CoinSpend {
    coin: Coin,
    puzzle_reveal: Program,
    solution: Program,
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl CoinSpend {
    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(cls: &Bound<'_, PyType>, py: Python<'_>, cs: Self) -> PyResult<PyObject> {
        // Convert result into potential child class
        let instance = cls.call1((cs.coin, cs.puzzle_reveal, cs.solution))?;

        Ok(instance.into_pyobject(py)?.unbind())
    }
}
