use crate::from_json_dict::FromJsonDict;
use crate::to_json_dict::ToJsonDict;
use chia_streamable_macro::Streamable;
use py_streamable::PyStreamable;

use chia::bytes::Bytes32;
use chia::chia_error;
use chia::streamable::Streamable;
use clvmr::sha2::{Digest, Sha256};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::convert::TryInto;

#[pyclass(unsendable)]
#[derive(Streamable, PyStreamable, Hash, Debug, Clone, Eq, PartialEq)]
pub struct Coin {
    #[pyo3(get)]
    parent_coin_info: Bytes32,
    #[pyo3(get)]
    puzzle_hash: Bytes32,
    #[pyo3(get)]
    amount: u64,
}

impl Coin {
    fn coin_id(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&self.parent_coin_info);
        hasher.update(&self.puzzle_hash);

        let amount_bytes = self.amount.to_be_bytes();
        if self.amount >= 0x8000000000000000_u64 {
            hasher.update(&[0_u8]);
            hasher.update(&amount_bytes);
        } else {
            let start = match self.amount {
                n if n >= 0x80000000000000_u64 => 0,
                n if n >= 0x800000000000_u64 => 1,
                n if n >= 0x8000000000_u64 => 2,
                n if n >= 0x80000000_u64 => 3,
                n if n >= 0x800000_u64 => 4,
                n if n >= 0x8000_u64 => 5,
                n if n >= 0x80_u64 => 6,
                n if n > 0 => 7,
                _ => 8,
            };
            hasher.update(&amount_bytes[start..]);
        }

        hasher.finalize().as_slice().try_into().unwrap()
    }
}

#[pymethods]
impl Coin {
    fn name<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
        Ok(PyBytes::new(py, &self.coin_id()))
    }
}
