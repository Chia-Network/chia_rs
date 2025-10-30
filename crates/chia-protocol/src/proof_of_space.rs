use crate::bytes::{Bytes, Bytes32};
use chia_bls::G1Element;
use chia_streamable_macro::streamable;
use chia_traits::chia_error;

#[streamable(no_json)]
pub struct ProofOfSpace {
    challenge: Bytes32,
    pool_public_key: Option<G1Element>,
    pool_contract_puzzle_hash: Option<Bytes32>,
    plot_public_key: G1Element,
    /// The 2 top bits determine the type of proof:
    /// 00 = v1 plot, the field store k-size
    /// 10 = v2 plot, the field store strength
    /// 01 = reserved
    /// 11 = reserved
    /// this field was renamed when adding support for v2 plots since the top
    /// bit now means whether it's v1 or v2. To stay backwards compabible with
    /// JSON serialization, we still serialize this as its original name
    #[cfg_attr(feature = "serde", serde(rename = "size", alias = "version_and_size"))]
    version_and_size: u8,
    proof: Bytes,
}

/// The k-size for v1 PoS, or strength if it's a v2 PoS
#[derive(Debug, PartialEq)]
pub enum PlotParam {
    KSize(u8),
    Strength(u8),
}

impl ProofOfSpace {
    pub fn param(&self) -> chia_error::Result<PlotParam> {
        match self.version_and_size & 0b1100_0000 {
            // valid v1 plot sizes are 32-50 (mainnet) and 18-50 (testnet)
            0b0000_0000 => Ok(PlotParam::KSize(self.version_and_size)),
            // valid v2 plot strength are 2-63
            0b1000_0000 => Ok(PlotParam::Strength(self.version_and_size & 0x3f)),
            _ => Err(chia_error::Error::InvalidPoSVersion),
        }
    }
}

#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pyclass(name = "PlotParam")]
pub struct PyPlotParam {
    #[pyo3(get)]
    pub size_v1: Option<u8>,
    #[pyo3(get)]
    pub strength_v2: Option<u8>,
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl PyPlotParam {
    #[staticmethod]
    fn make_v1(s: u8) -> Self {
        assert!(s < 64);
        Self {
            size_v1: Some(s),
            strength_v2: None,
        }
    }

    #[staticmethod]
    fn make_v2(s: u8) -> Self {
        assert!(s < 64);
        Self {
            size_v1: None,
            strength_v2: Some(s),
        }
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ProofOfSpace {
    #[pyo3(name = "param")]
    fn py_param(&self) -> PyResult<PyPlotParam> {
        match self.param()? {
            PlotParam::KSize(s) => Ok(PyPlotParam {
                size_v1: Some(s),
                strength_v2: None,
            }),
            PlotParam::Strength(s) => Ok(PyPlotParam {
                size_v1: None,
                strength_v2: Some(s),
            }),
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
    #[case(0x00, Ok(PlotParam::KSize(0)))]
    #[case(0x01, Ok(PlotParam::KSize(1)))]
    #[case(0x08, Ok(PlotParam::KSize(8)))]
    #[case(0x3f, Ok(PlotParam::KSize(0x3f)))]
    #[case(0x80, Ok(PlotParam::Strength(0)))]
    #[case(0x81, Ok(PlotParam::Strength(1)))]
    #[case(0x80 + 28, Ok(PlotParam::Strength(28)))]
    #[case(0x80 + 30, Ok(PlotParam::Strength(30)))]
    #[case(0x80 + 32, Ok(PlotParam::Strength(32)))]
    #[case(0xff, Err(chia_error::Error::InvalidPoSVersion))]
    #[case(0x7f, Err(chia_error::Error::InvalidPoSVersion))]
    fn proof_of_space_size(#[case] size_field: u8, #[case] expect: chia_traits::Result<PlotParam>) {
        let pos = ProofOfSpace::new(
            Bytes32::from(b"abababababababababababababababab"),
            None,
            None,
            G1Element::default(),
            size_field,
            Bytes::from(vec![]),
        );

        assert_eq!(pos.param(), expect);
    }
}
