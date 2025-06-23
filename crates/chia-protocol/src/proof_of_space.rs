use crate::bytes::{Bytes, Bytes32};
use chia_bls::G1Element;
use chia_streamable_macro::streamable;

#[streamable(no_json)]
pub struct ProofOfSpace {
    challenge: Bytes32,
    pool_public_key: Option<G1Element>,
    pool_contract_puzzle_hash: Option<Bytes32>,
    plot_public_key: G1Element,
    // this field was renamed when adding support for v2 plots since the top
    // bit now means whether it's v1 or v2. To stay backwards compabible with
    // JSON serialization, we still serialize this as its original name
    #[cfg_attr(feature = "serde", serde(rename = "size", alias = "version_and_size"))]
    version_and_size: u8,
    proof: Bytes,
}

#[derive(Debug, PartialEq)]
pub enum PlotSize {
    V1(u8),
    V2(u8),
}

impl ProofOfSpace {
    pub fn size(&self) -> PlotSize {
        if (self.version_and_size & 0x80) == 0 {
            PlotSize::V1(self.version_and_size)
        } else {
            PlotSize::V2(self.version_and_size & 0x7f)
        }
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pyclass(name = "PlotSize")]
pub struct PyPlotSize {
    #[pyo3(get)]
    pub size_v1: Option<u8>,
    #[pyo3(get)]
    pub size_v2: Option<u8>,
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl PyPlotSize {
    #[staticmethod]
    fn make_v1(s: u8) -> Self {
        Self {
            size_v1: Some(s),
            size_v2: None,
        }
    }

    #[staticmethod]
    fn make_v2(s: u8) -> Self {
        Self {
            size_v1: None,
            size_v2: Some(s),
        }
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ProofOfSpace {
    #[pyo3(name = "size")]
    fn py_size(&self) -> PyPlotSize {
        match self.size() {
            PlotSize::V1(s) => PyPlotSize {
                size_v1: Some(s),
                size_v2: None,
            },
            PlotSize::V2(s) => PyPlotSize {
                size_v1: None,
                size_v2: Some(s),
            },
        }
    }
}

#[cfg(feature = "py-bindings")]
impl ToJsonDict for ProofOfSpace {
    fn to_json_dict(&self, py: pyo3::Python<'_>) -> pyo3::PyResult<pyo3::PyObject> {
        use pyo3::prelude::PyDictMethods;
        let ret = pyo3::types::PyDict::new(py);

        ret.set_item("challenge", self.challenge.to_json_dict(py)?)?;
        ret.set_item("pool_public_key", self.pool_public_key.to_json_dict(py)?)?;
        ret.set_item(
            "pool_contract_puzzle_hash",
            self.pool_contract_puzzle_hash.to_json_dict(py)?,
        )?;
        ret.set_item("plot_public_key", self.plot_public_key.to_json_dict(py)?)?;

        // "size" was the original name of this field. We keep it to remain backwards compatible
        ret.set_item("size", self.version_and_size.to_json_dict(py)?)?;
        ret.set_item("proof", self.proof.to_json_dict(py)?)?;

        Ok(ret.into())
    }
}

#[cfg(feature = "py-bindings")]
impl FromJsonDict for ProofOfSpace {
    fn from_json_dict(o: &pyo3::Bound<'_, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::prelude::PyAnyMethods;
        Ok(Self {
            challenge: <Bytes32 as FromJsonDict>::from_json_dict(&o.get_item("challenge")?)?,
            pool_public_key: <Option<G1Element> as FromJsonDict>::from_json_dict(
                &o.get_item("pool_public_key")?,
            )?,
            pool_contract_puzzle_hash: <Option<Bytes32> as FromJsonDict>::from_json_dict(
                &o.get_item("pool_contract_puzzle_hash")?,
            )?,
            plot_public_key: <G1Element as FromJsonDict>::from_json_dict(
                &o.get_item("plot_public_key")?,
            )?,
            version_and_size: <u8 as FromJsonDict>::from_json_dict(&o.get_item("size")?)?,
            proof: <Bytes as FromJsonDict>::from_json_dict(&o.get_item("proof")?)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::needless_pass_by_value)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0x00, PlotSize::V1(0))]
    #[case(0x01, PlotSize::V1(1))]
    #[case(0x08, PlotSize::V1(8))]
    #[case(0x7f, PlotSize::V1(0x7f))]
    #[case(0x80, PlotSize::V2(0))]
    #[case(0x81, PlotSize::V2(1))]
    #[case(0x80 + 28, PlotSize::V2(28))]
    #[case(0x80 + 30, PlotSize::V2(30))]
    #[case(0x80 + 32, PlotSize::V2(32))]
    #[case(0xff, PlotSize::V2(0x7f))]
    fn proof_of_space_size(#[case] size_field: u8, #[case] expect: PlotSize) {
        let pos = ProofOfSpace::new(
            Bytes32::from(b"abababababababababababababababab"),
            None,
            None,
            G1Element::default(),
            size_field,
            Bytes::from(vec![]),
        );

        assert_eq!(pos.size(), expect);
    }
}
