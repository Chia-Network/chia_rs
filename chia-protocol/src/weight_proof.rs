use crate::streamable_struct;
use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::ProofOfSpace;
use crate::Streamable;
use crate::{VDFInfo, VDFProof};

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct! (SubSlotData {
    proof_of_space: Option<ProofOfSpace>,
    cc_signage_point: Option<VDFProof>,
    cc_infusion_point: Option<VDFProof>,
    icc_infusion_point: Option<VDFProof>,
    cc_sp_vdf_info: Option<VDFInfo>,
    signage_point_index: Option<u8>,
    cc_slot_end: Option<VDFProof>,
    icc_slot_end: Option<VDFProof>,
    cc_slot_end_info: Option<VDFInfo>,
    icc_slot_end_info: Option<VDFInfo>,
    cc_ip_vdf_info: Option<VDFInfo>,
    icc_ip_vdf_info: Option<VDFInfo>,
    total_iters: Option<u128>,
});

streamable_struct! (SubEpochChallengeSegment {
    sub_epoch_n: u32,
    sub_slots: Vec<SubSlotData>,
    rc_slot_end_info: Option<VDFInfo>,
});

streamable_struct! (SubEpochSegments {
    challenge_segments: Vec<SubEpochChallengeSegment>,
});
