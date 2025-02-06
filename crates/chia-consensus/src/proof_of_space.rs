use chia_protocol::{Bytes32, ProofOfSpace};
use chiapos::validate_proof;

pub fn get_quality_string(pos: &ProofOfSpace, plot_id: Bytes32) -> Option<Bytes32> {
    let mut quality = [0; 32];
    if validate_proof(
        &plot_id.to_bytes(),
        pos.size,
        &pos.challenge.to_bytes(),
        &pos.proof,
        &mut quality,
    ) {
        Some(Bytes32::from(quality))
    } else {
        None
    }
}
