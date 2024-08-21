use chia_protocol::{ClassgroupElement, VDFInfo, VDFProof};
use chiavdf::{create_discriminant, verify_n_wesolowski};

use crate::consensus_constants::ConsensusConstants;

pub fn validate_vdf_proof(
    proof: &VDFProof,
    input_el: &ClassgroupElement,
    info: &VDFInfo,
    constants: &ConsensusConstants,
) -> bool {
    if proof.witness_type + 1 > constants.max_vdf_witness_size {
        return false;
    }

    let mut discriminant = vec![0; constants.discriminant_size_bits as usize / 8];
    create_discriminant(&info.challenge, &mut discriminant);

    let proof_buf = [info.output.data.as_slice(), proof.witness.as_slice()].concat();

    verify_n_wesolowski(
        &discriminant,
        &input_el.data,
        &proof_buf,
        info.number_of_iterations,
        proof.witness_type as u64,
    )
}
