use crate::bytes::{Bytes, Bytes32};
use chia_bls::G1Element;
use chia_sha2::Sha256;
use chia_streamable_macro::streamable;
use chia_traits::{Error, Result, Streamable};
use std::io::Cursor;

// This structure was updated for v2 proof-of-space, in a backwards compatible
// way. The Option types are serialized as 1 byte to indicate whether the value
// is set or not. Only 1 bit out of 8 are used. The byte prefix for
// pool_contract_puzzle_hash is used to indicate whether this is a v1 or v2
// proof.
#[streamable(no_streamable)]
pub struct ProofOfSpace {
    challenge: Bytes32,
    pool_public_key: Option<G1Element>,
    pool_contract_puzzle_hash: Option<Bytes32>,
    plot_public_key: G1Element,

    // this is 0 for v1 proof-of-space and 1 for v2. The version is encoded as
    // part of the 8 bits prefix, indicating whether pool_contract_puzzle_hash
    // is set or not
    version: u8,

    // These are set for v2 proofs and all zero for v1 proofs
    plot_index: u16,
    meta_group: u8,
    strength: u8,

    // this is set for v1 proofs, and zero for v2 proofs
    size: u8,

    proof: Bytes,
}

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[cfg(feature = "py-bindings")]
#[pyclass(name = "PlotParam")]
pub struct PyPlotParam {
    #[pyo3(get)]
    pub size_v1: Option<u8>,
    #[pyo3(get)]
    pub strength_v2: Option<u8>,
    #[pyo3(get)]
    pub plot_index: u16,
    #[pyo3(get)]
    pub meta_group: u8,
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
            plot_index: 0,
            meta_group: 0,
        }
    }

    #[staticmethod]
    fn make_v2(plot_index: u16, meta_group: u8, strength: u8) -> Self {
        assert!(strength < 64);
        Self {
            size_v1: None,
            strength_v2: Some(strength),
            plot_index,
            meta_group,
        }
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl ProofOfSpace {
    #[pyo3(name = "param")]
    fn py_param(&self) -> PyPlotParam {
        match self.version {
            0 => PyPlotParam {
                size_v1: Some(self.size),
                strength_v2: None,
                plot_index: 0,
                meta_group: 0,
            },
            1 => PyPlotParam {
                size_v1: None,
                strength_v2: Some(self.strength),
                plot_index: self.plot_index,
                meta_group: self.meta_group,
            },
            _ => {
                panic!("invalid proof-of-space version {}", self.version);
            }
        }
    }
}

// ProofOfSpace was updated in Chia 3.0 to support v2 proofs. In order to stay
// backwards compatible with the network protocol and the block hashes of
// previous versions, for v1 proofs, some care has to be taken.
// Optional fields are serialized with a 1-byte prefix indicating whether the
// field is set or not. This byte is either 0 or 1. This leaves 7 unused bits.
// We use bit 2 in the byte prefix for the pool_contract_puzzle_hash field to
// indicate whether this is a v2 proof or not. v1 proofs leave this bit as 0,
// and thus remain backwards compatible. V2 proofs set it to 1, which alters
// which fields are serialized. e.g. we no longer include size (k) of the plot
// since v2 plots have a fixed size.
impl Streamable for ProofOfSpace {
    fn update_digest(&self, digest: &mut Sha256) {
        self.challenge.update_digest(digest);
        self.pool_public_key.update_digest(digest);

        if self.version == 0 {
            self.pool_contract_puzzle_hash.update_digest(digest);
            self.plot_public_key.update_digest(digest);
            self.size.update_digest(digest);
        } else if self.version == 1 {
            if let Some(pool_contract) = self.pool_contract_puzzle_hash {
                0b11_u8.update_digest(digest);
                pool_contract.update_digest(digest);
            } else {
                0b10_u8.update_digest(digest);
            }

            self.plot_public_key.update_digest(digest);
            self.plot_index.update_digest(digest);
            self.meta_group.update_digest(digest);
            self.strength.update_digest(digest);
        } else {
            panic!("version field must be 0 or 1, but it's {}", self.version);
        }

        self.proof.update_digest(digest);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.challenge.stream(out)?;
        self.pool_public_key.stream(out)?;

        if self.version == 0 {
            self.pool_contract_puzzle_hash.stream(out)?;
            self.plot_public_key.stream(out)?;
            self.size.stream(out)?;
        } else if self.version == 1 {
            if let Some(pool_contract) = self.pool_contract_puzzle_hash {
                0b11_u8.stream(out)?;
                pool_contract.stream(out)?;
            } else {
                0b10_u8.stream(out)?;
            }

            self.plot_public_key.stream(out)?;
            self.plot_index.stream(out)?;
            self.meta_group.stream(out)?;
            self.strength.stream(out)?;
        } else {
            return Err(Error::InvalidPoSVersion);
        }

        self.proof.stream(out)
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let challenge = <Bytes32 as Streamable>::parse::<TRUSTED>(input)?;
        let pool_public_key = <Option<G1Element> as Streamable>::parse::<TRUSTED>(input)?;

        let prefix = <u8 as Streamable>::parse::<TRUSTED>(input)?;
        let version = u8::from((prefix & 0b10) != 0);
        let pool_contract_puzzle_hash = if (prefix & 1) != 0 {
            Some(<Bytes32 as Streamable>::parse::<TRUSTED>(input)?)
        } else {
            None
        };

        let plot_public_key = <G1Element as Streamable>::parse::<TRUSTED>(input)?;

        if version == 0 {
            let size = <u8 as Streamable>::parse::<TRUSTED>(input)?;
            let proof = <Bytes as Streamable>::parse::<TRUSTED>(input)?;

            Ok(ProofOfSpace {
                challenge,
                pool_public_key,
                pool_contract_puzzle_hash,
                plot_public_key,
                version,
                plot_index: 0,
                meta_group: 0,
                strength: 0,
                size,
                proof,
            })
        } else {
            let plot_index = <u16 as Streamable>::parse::<TRUSTED>(input)?;
            let meta_group = <u8 as Streamable>::parse::<TRUSTED>(input)?;
            let strength = <u8 as Streamable>::parse::<TRUSTED>(input)?;
            let proof = <Bytes as Streamable>::parse::<TRUSTED>(input)?;

            Ok(ProofOfSpace {
                challenge,
                pool_public_key,
                pool_contract_puzzle_hash,
                plot_public_key,
                version,
                plot_index,
                meta_group,
                strength,
                size: 0,
                proof,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::needless_pass_by_value)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(0, 18, Ok(18))]
    #[case(0, 28, Ok(28))]
    #[case(0, 38, Ok(38))]
    #[case(1, 18, Ok(0))]
    #[case(1, 28, Ok(0))]
    #[case(1, 38, Ok(0))]
    #[case(2, 18, Err(Error::InvalidPoSVersion))]
    fn proof_of_space_size(#[case] version: u8, #[case] size: u8, #[case] expect: Result<u8>) {
        let pos = ProofOfSpace::new(
            Bytes32::from(b"abababababababababababababababab"),
            None,
            None,
            G1Element::default(),
            version,
            0,
            0,
            0,
            size,
            Bytes::from(vec![]),
        );

        match pos.to_bytes() {
            Ok(buf) => {
                let new_pos =
                    ProofOfSpace::parse::<false>(&mut Cursor::<&[u8]>::new(&buf)).expect("parse()");
                assert_eq!(new_pos.size, expect.unwrap());
            }
            Err(e) => {
                assert_eq!(e, expect.unwrap_err());
            }
        }
    }
}
