use chia_streamable_macro::streamable;

use crate::{Bytes32, TwoOption};

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use pyo3::pymethods;

#[streamable(no_json)]
pub struct SubEpochSummary {
    prev_subepoch_summary_hash: Bytes32,
    reward_chain_hash: Bytes32, // hash of reward chain at end of last segment
    num_blocks_overflow: u8,    // How many more blocks than 384*(N-1)
    new_difficulty: Option<u64>, // Only once per epoch (diff adjustment)
    // Only once per epoch (diff adjustment)
    new_sub_slot_iters_and_merkle_root: TwoOption<u64, Bytes32>,
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl SubEpochSummary {
    #[getter]
    pub fn new_sub_slot_iters(&self) -> Option<u64> {
        self.new_sub_slot_iters_and_merkle_root.0
    }

    #[getter]
    pub fn merkle_root(&self) -> Option<Bytes32> {
        self.new_sub_slot_iters_and_merkle_root.1
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for SubEpochSummary {
    fn to_json_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        use pyo3::prelude::PyDictMethods;
        let ret = pyo3::types::PyDict::new(py);

        ret.set_item(
            "prev_subepoch_summary_hash",
            self.prev_subepoch_summary_hash.to_json_dict(py)?,
        )?;
        ret.set_item(
            "reward_chain_hash",
            self.reward_chain_hash.to_json_dict(py)?,
        )?;
        ret.set_item(
            "num_blocks_overflow",
            self.num_blocks_overflow.to_json_dict(py)?,
        )?;
        ret.set_item("new_difficulty", self.new_difficulty.to_json_dict(py)?)?;
        ret.set_item(
            "new_slot_iters",
            self.new_sub_slot_iters_and_merkle_root.0.to_json_dict(py)?,
        )?;
        ret.set_item(
            "merkle_root",
            self.new_sub_slot_iters_and_merkle_root.1.to_json_dict(py)?,
        )?;
        Ok(ret.into())
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for SubEpochSummary {
    fn from_json_dict(o: &pyo3::Bound<'_, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::prelude::PyAnyMethods;
        Ok(Self {
            prev_subepoch_summary_hash: Bytes32::from_json_dict(
                &o.get_item("prev_subepoch_summary_hash")?,
            )?,
            reward_chain_hash: Bytes32::from_json_dict(&o.get_item("reward_chain_hash")?)?,
            num_blocks_overflow: u8::from_json_dict(&o.get_item("num_blocks_overflow")?)?,
            new_difficulty: Option::<u64>::from_json_dict(&o.get_item("new_difficulty")?)?,
            new_sub_slot_iters_and_merkle_root: TwoOption(
                Option::<u64>::from_json_dict(&o.get_item("new_sub_slot_iters")?)?,
                Option::<Bytes32>::from_json_dict(&o.get_item("merkle_root")?)?,
            ),
        })
    }
}
