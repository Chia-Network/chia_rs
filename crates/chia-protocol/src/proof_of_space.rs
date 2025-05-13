use crate::bytes::{Bytes, Bytes32};
use chia_bls::G1Element;
use chia_streamable_macro::streamable;

#[streamable]
pub struct ProofOfSpace {
    challenge: Bytes32,
    pool_public_key: Option<G1Element>,
    pool_contract_puzzle_hash: Option<Bytes32>,
    plot_public_key: G1Element,
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
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ProofOfSpace {
    fn size_v1(&self) -> Option<u8> {
        match self.size() {
            PlotSize::V1(s) => Some(s),
            PlotSize::V2(_) => None,
        }
    }

    fn size_v2(&self) -> Option<u8> {
        match self.size() {
            PlotSize::V1(_) => None,
            PlotSize::V2(s) => Some(s),
        }
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
