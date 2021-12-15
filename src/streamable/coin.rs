use bincode::Options;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde::{Deserialize, Serialize};

type Bytes32 = [u8; 32];

#[pyclass(subclass, unsendable)]
#[derive(Serialize, Deserialize, Debug)]
pub struct Coin {
    #[pyo3(get)]
    parent_coin_info: Bytes32,
    #[pyo3(get)]
    puzzle_hash: Bytes32,
    #[pyo3(get)]
    amount: u64,
}

#[pymethods]
impl Coin {
    #[new]
    fn new(parent_coin_info: Bytes32, puzzle_hash: Bytes32, amount: u64) -> Self {
        Self {
            parent_coin_info,
            puzzle_hash,
            amount,
        }
    }

    #[staticmethod]
    pub fn from_bytes(blob: &[u8]) -> Self {
        let chia = bincode::DefaultOptions::new()
            .with_chia_int_encoding()
            .allow_trailing_bytes()
            .with_big_endian();
        chia.deserialize(blob).unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let chia = bincode::DefaultOptions::new()
            .with_chia_int_encoding()
            .allow_trailing_bytes()
            .with_big_endian();
        chia.serialize(&self).unwrap()
    }

    pub fn __bytes__<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
        Ok(PyBytes::new(py, &self.to_bytes()))
    }
}
