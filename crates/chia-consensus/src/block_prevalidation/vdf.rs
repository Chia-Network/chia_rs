use chia_protocol::{ClassgroupElement, VDFInfo, VDFProof};

use crate::consensus_constants::ConsensusConstants;

/// If target_vdf_info is passed in, it is compared with info.
pub fn validate_vdf(
    proof: VDFProof,
    constants: ConsensusConstants,
    input_element: ClassgroupElement,
    info: VDFInfo,
    target_vdf_info: Option<VDFInfo>,
) -> bool {
    if let Some(target_vdf_info) = target_vdf_info {
        if target_vdf_info != info {
            return false;
        }
    }

    if proof.witness_type + 1 > constants.max_vdf_witness_size {
        return false;
    }

    todo!()
}
