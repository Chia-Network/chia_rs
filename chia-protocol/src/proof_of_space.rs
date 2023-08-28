use crate::bytes::{Bytes, Bytes32};
use crate::streamable_struct;
use chia_bls::G1Element;
use chia_streamable_macro::Streamable;

streamable_struct! (ProofOfSpace {
    challenge: Bytes32,
    pool_public_key: Option<G1Element>,
    pool_contract_puzzle_hash: Option<Bytes32>,
    plot_public_key: G1Element,
    size: u8,
    proof: Bytes,
});
