use chia_streamable_macro::streamable;

#[streamable]
pub struct PartialProof {
    #[cfg_attr(feature = "serde", serde(with = "serde_arrays"))]
    proof_fragments: [u64; 64],
}
