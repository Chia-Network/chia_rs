use crate::bytes::{Bytes48, Bytes96};
use chia_streamable_macro::Streamable;
use clvm_traits::{FromClvm, ToClvm};

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use chia_traits::{FromJsonDict, ToJsonDict};

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg_attr(feature = "py-bindings", pyclass(frozen), derive(PyStreamable))]
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq, FromClvm, ToClvm)]
#[clvm(tuple)]
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
#[derive(Streamable, Hash, Debug, Clone, Eq, PartialEq, FromClvm, ToClvm)]
#[clvm(tuple)]
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

#[cfg(test)]
mod tests {
    use clvmr::{
        serde::{node_from_bytes, node_to_bytes},
        Allocator,
    };

    use super::*;

    #[test]
    fn g1_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "b0b8f7dd239557ff8c49d338f89ac1a258a863fa52cd0a502e3aaae4b6738ba39ac8d982215aa3fa16bc5f8cb7e44b954d";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let g1 = G1Element::from_clvm(a, ptr).unwrap();

        let round_trip = g1.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }

    #[test]
    fn g2_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "c06091c3d0504c2c5e02091f92cf0c3f79f2d7350656b0dc554dfc94f7e256b53d415e1a15108e329004ff1c5e91e24b445d18e52b2777e9a01a7a12d7f69a9df30c6fe3c196bdc5aa8072ea23d0edb6422253bb02d560bce721a459e6cf9e847aed";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let g2 = G2Element::from_clvm(a, ptr).unwrap();

        let round_trip = g2.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }
}
