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

pub fn compute_plot_id_v1(
    plot_pk: &G1Element,
    pool_pk: Option<&G1Element>,
    pool_contract: Option<&Bytes32>,
) -> Bytes32 {
    let mut ctx = Sha256::new();
    // plot_id = sha256( ( pool_pk | contract_ph) + plot_pk)
    if let Some(pool_pk) = pool_pk {
        pool_pk.update_digest(&mut ctx);
    } else if let Some(contract_ph) = pool_contract {
        contract_ph.update_digest(&mut ctx);
    } else {
        panic!("invalid proof of space. Neither pool pk nor contract puzzle hash set");
    }
    plot_pk.update_digest(&mut ctx);
    ctx.finalize().into()
}

pub fn compute_plot_id_v2(
    strength: u8,
    plot_pk: &G1Element,
    pool_pk: Option<&G1Element>,
    pool_contract: Option<&Bytes32>,
    plot_index: u16,
    meta_group: u8,
) -> Bytes32 {
    let mut ctx = Sha256::new();
    // plot_group_id = sha256( 2 + strength + plot_pk + (pool_pk | contract_ph) )
    // plot_id = sha256( plot_group_id + plot_index + meta_group)
    let version = 2_u8;
    let mut group_ctx = Sha256::new();
    version.update_digest(&mut group_ctx);
    strength.update_digest(&mut group_ctx);
    plot_pk.update_digest(&mut group_ctx);
    if let Some(pool_pk) = pool_pk {
        pool_pk.update_digest(&mut group_ctx);
    } else if let Some(contract_ph) = pool_contract {
        contract_ph.update_digest(&mut group_ctx);
    } else {
        panic!(
            "failed precondition of compute_plot_id_2(). Either pool-public-key or pool-contract-hash must be specified"
        );
    }
    let plot_group_id: Bytes32 = group_ctx.finalize().into();

    plot_group_id.update_digest(&mut ctx);
    plot_index.update_digest(&mut ctx);
    meta_group.update_digest(&mut ctx);
    ctx.finalize().into()
}

impl ProofOfSpace {
    pub fn compute_plot_id(&self) -> Bytes32 {
        if self.version == 0 {
            // v1 proofs
            compute_plot_id_v1(
                &self.plot_public_key,
                self.pool_public_key.as_ref(),
                self.pool_contract_puzzle_hash.as_ref(),
            )
        } else if self.version == 1 {
            // v2 proofs
            compute_plot_id_v2(
                self.strength,
                &self.plot_public_key,
                self.pool_public_key.as_ref(),
                self.pool_contract_puzzle_hash.as_ref(),
                self.plot_index,
                self.meta_group,
            )
        } else {
            panic!("unknown proof version: {}", self.version);
        }
    }

    /// returns the quality string of the v2 proof of space.
    /// returns None if this is a v1 proof or if the proof is invalid.
    pub fn quality_string(&self) -> Option<Bytes32> {
        if self.version != 1 {
            return None;
        }

        let k_size = (self.proof.len() * 8 / 128) as u8;
        let plot_id = self.compute_plot_id().to_bytes();
        chia_pos2::validate_proof_v2(
            &plot_id,
            k_size,
            &self.challenge.to_bytes(),
            self.strength,
            self.proof.as_slice(),
        )
        .map(|quality| {
            let mut sha256 = Sha256::new();
            sha256.update(chia_pos2::serialize_quality(
                &quality.chain_links,
                self.strength,
            ));
            sha256.finalize().into()
        })
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

    #[pyo3(name = "compute_plot_id")]
    pub fn py_compute_plot_id(&self) -> Bytes32 {
        self.compute_plot_id()
    }

    #[pyo3(name = "quality_string")]
    pub fn py_quality_string(&self) -> Option<Bytes32> {
        self.quality_string()
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
    use hex_literal::hex;
    use rstest::rstest;

    fn plot_pk() -> G1Element {
        const PLOT_PK_BYTES: [u8; 48] = hex!(
            "96b35c22adf93068c9536e016e88251ad715a591d8deabb60917d9c495f45a220ca56b906793c27778d5f7f71fb50b94"
        );
        G1Element::from_bytes(&PLOT_PK_BYTES).expect("PLOT_PK_BYTES is valid")
    }

    fn pool_pk() -> G1Element {
        const POOL_PK_BYTES: [u8; 48] = hex!(
            "ac6e995e0f9c307853fa5c79e571de5ec2f2d45e5c2641c0847fef8041916e4d07d5a9200d5aa92ceac3b1bf41ce93b2"
        );
        G1Element::from_bytes(&POOL_PK_BYTES).expect("POOL_PK_BYTES is valid")
    }

    // these are regression tests and test vectors for plot ID computations
    #[rstest]
    #[case("pool_pk", hex!("e185d4ec721ec060eb5833ec07d802fc69a43ed45dd59d7f20c58494421e0270"))]
    #[case("contract_ph", hex!("4e196e2fb1fc4c85fc48b30c1e585dc0bee08451895909b0ae2db63e2788ab82"))]
    fn test_compute_plot_id_v1(#[case] variant: &str, #[case] expected: [u8; 32]) {
        let (pool_pk, pool_contract) = match variant {
            "pool_pk" => (Some(pool_pk()), None),
            "contract_ph" => (None, Some(Bytes32::new([1u8; 32]))),
            _ => panic!("unknown v1 variant: {variant}"),
        };
        let result = compute_plot_id_v1(&plot_pk(), pool_pk.as_ref(), pool_contract.as_ref());
        assert_eq!(result, Bytes32::new(expected));
    }

    #[rstest]
    #[case(0, 0, 0, "pool_pk", hex!("d2d7c5e9e2955b33cf99058fc8ac0706b284d81768ce19e789bbbdd42eb9f6a1"))]
    #[case(10, 256, 7, "pool_pk", hex!("b9fa5318770889a8ab4af143e9dac806b98e1033a128d3f50d18579c6cb78e9f"))]
    #[case(0, 0, 0, "contract_ph", hex!("7390a21f1793f0920d698662194a1c5311c8ae2dd96c89778f86d88bbeb069e1"))]
    #[case(5, 100, 3, "contract_ph", hex!("fa5462dd1c20194027119600d5eae77de1283cff97efb49522f563fa2f5608ec"))]
    fn test_compute_plot_id_v2(
        #[case] strength: u8,
        #[case] plot_index: u16,
        #[case] meta_group: u8,
        #[case] variant: &str,
        #[case] expected: [u8; 32],
    ) {
        let (pool_pk, pool_contract) = match variant {
            "pool_pk" => (Some(pool_pk()), None),
            "contract_ph" => (None, Some(Bytes32::new([1u8; 32]))),
            _ => panic!("unknown v2 variant: {variant}"),
        };
        let result = compute_plot_id_v2(
            strength,
            &plot_pk(),
            pool_pk.as_ref(),
            pool_contract.as_ref(),
            plot_index,
            meta_group,
        );
        assert_eq!(result, Bytes32::new(expected));
    }

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
