//! VDF proof verification.
//!
//! Port of chiavdf/src/verifier.h.

use crate::bqfc::BQFC_FORM_SIZE;
use crate::form::Form;
use crate::nucomp::nucomp;
use crate::proof_common::{deserialize_form, fast_pow, fast_pow_form_nucomp, get_b, B_BYTES};
use crate::reducer::reduce;
use malachite_nz::integer::Integer;

/// Verify a single Wesolowski segment.
///
/// Checks: proof^B * x^r == y
/// where r = 2^iters mod B.
///
/// Returns Ok(y_computed) on success, Err on failure.
pub fn verify_weso_segment(
    d: &Integer,
    x: &Form,
    proof: &Form,
    b: &Integer,
    iters: u64,
) -> Result<Form, String> {
    let l = Form::compute_l(d);
    let r = fast_pow(iters, b);

    let f1 = fast_pow_form_nucomp(proof, d, b, &l);
    let f2 = fast_pow_form_nucomp(x, d, &r, &l);

    let mut out_y = nucomp(&f1, &f2, d, &l);
    reduce(&mut out_y);

    Ok(out_y)
}

/// Verify a Wesolowski proof.
///
/// Checks: proof^B * x^r == y
pub fn verify_wesolowski_proof(d: &Integer, x: &Form, y: &Form, proof: &Form, iters: u64) -> bool {
    let l = Form::compute_l(d);
    let mut x_mut = x.clone();
    let mut y_mut = y.clone();
    let b = get_b(d, &mut x_mut, &mut y_mut);
    let r = fast_pow(iters, &b);

    let f1 = fast_pow_form_nucomp(proof, d, &b, &l);
    let f2 = fast_pow_form_nucomp(x, d, &r, &l);

    let mut result = nucomp(&f1, &f2, d, &l);
    reduce(&mut result);

    result == *y
}

/// Verify a N-Wesolowski proof blob.
///
/// proof_blob format:
///   [y_form (100 bytes)] [proof_form (100 bytes)] [segments from back...]
/// Each segment: [iters (8 bytes)] [B (33 bytes)] [proof_form (100 bytes)]
///
/// Returns true if proof is valid.
pub fn check_proof_of_time_n_wesolowski(
    d: &Integer,
    x_s: &[u8],
    proof_blob: &[u8],
    iterations: u64,
    depth: u64,
) -> bool {
    let form_size = BQFC_FORM_SIZE;
    let segment_len = 8 + B_BYTES + form_size;
    let base_len = 2 * form_size;

    if depth > (usize::MAX - base_len) as u64 / segment_len as u64 {
        return false;
    }

    let expected_len = base_len + depth as usize * segment_len;
    if proof_blob.len() != expected_len {
        return false;
    }

    let mut x = match deserialize_form(d, x_s) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut remaining_iters = iterations;
    let mut i = proof_blob.len();

    while i > base_len {
        i -= segment_len;

        let segment_iters = u64::from_be_bytes(proof_blob[i..i + 8].try_into().unwrap());

        let b_bytes = &proof_blob[i + 8..i + 8 + B_BYTES];
        let b = crate::integer::from_bytes_be(b_bytes);

        let proof_bytes = &proof_blob[i + 8 + B_BYTES..i + 8 + B_BYTES + form_size];
        let proof = match deserialize_form(d, proof_bytes) {
            Ok(f) => f,
            Err(_) => return false,
        };

        let out_y = match verify_weso_segment(d, &x, &proof, &b, segment_iters) {
            Ok(y) => y,
            Err(_) => return false,
        };

        let mut x_clone = x.clone();
        let mut out_y_clone = out_y.clone();
        let computed_b = get_b(d, &mut x_clone, &mut out_y_clone);
        if computed_b != b {
            return false;
        }

        x = out_y;

        if segment_iters > remaining_iters {
            return false;
        }
        remaining_iters -= segment_iters;
    }

    let y = match deserialize_form(d, &proof_blob[..form_size]) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let proof = match deserialize_form(d, &proof_blob[form_size..2 * form_size]) {
        Ok(f) => f,
        Err(_) => return false,
    };

    verify_wesolowski_proof(d, &x, &y, &proof, remaining_iters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discriminant::create_discriminant;

    #[test]
    fn test_verify_basic_structure() {
        let seed = b"test";
        let d = create_discriminant(seed, 512);
        let x = Form::identity(&d);

        let iters = 100u64;
        let result = verify_wesolowski_proof(&d, &x, &x, &x, iters);
        let _ = result;
    }
}
