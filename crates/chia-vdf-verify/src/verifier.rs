//! VDF proof verification.
//!
//! Port of chiavdf/src/verifier.h.

use crate::bqfc::{BQFC_FORM_SIZE, BQFC_MAX_D_BITS};
use crate::form::Form;
use crate::integer::num_bits;
use crate::nucomp::nucomp;
use crate::proof_common::{
    B_BYTES, deserialize_form, fast_pow, fast_pow_form_nucomp, get_b, serialize_form,
};
use crate::reducer::reduce;
use malachite_nz::integer::Integer;

/// True when `num_bits(d)` is in `1..=BQFC_MAX_D_BITS` (chiavdf `IsDiscriminantInRange`).
pub fn is_discriminant_in_range(d: &Integer) -> bool {
    let bits = num_bits(d);
    bits > 0 && bits <= BQFC_MAX_D_BITS
}

/// True when `disc_size_bits` is in `1..=BQFC_MAX_D_BITS` (chiavdf `IsDiscSizeBitsInRange`).
pub fn is_disc_size_bits_in_range(disc_size_bits: usize) -> bool {
    disc_size_bits > 0 && disc_size_bits <= BQFC_MAX_D_BITS
}

/// Verify a single Wesolowski segment.
///
/// Checks: proof^B * x^r == y where r = 2^iters mod B, and B == GetB(D, x, y).
/// Returns Ok(y) on success, Err on B mismatch.
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

    let mut x_clone = x.clone();
    let mut y_clone = out_y.clone();
    let computed_b = get_b(d, &mut x_clone, &mut y_clone);
    if computed_b != *b {
        return Err("B mismatch in Wesolowski segment".to_string());
    }

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

/// Shared N-Wesolowski segment walk (chiavdf `CheckProofOfTimeNWesolowskiCommon`).
///
/// Walks segments from the end of `proof_blob` down to `last_segment`, updating
/// `x` and decrementing `iterations`. When `skip_check` is true, skips B
/// verification (used by `get_b_from_proof`).
fn check_proof_of_time_n_wesolowski_common(
    d: &Integer,
    x: &mut Form,
    proof_blob: &[u8],
    iterations: &mut u64,
    last_segment: usize,
    skip_check: bool,
) -> bool {
    let form_size = BQFC_FORM_SIZE;
    let segment_len = 8 + B_BYTES + form_size;

    if proof_blob.len() < last_segment {
        return false;
    }
    if !(proof_blob.len() - last_segment).is_multiple_of(segment_len) {
        return false;
    }

    let strict = false;
    let mut i = proof_blob.len();
    let l = Form::compute_l(d);

    while i > last_segment {
        i -= segment_len;

        let segment_iters = u64::from_be_bytes(proof_blob[i..i + 8].try_into().unwrap());
        let b_bytes = &proof_blob[i + 8..i + 8 + B_BYTES];
        let b = crate::integer::from_bytes_be(b_bytes);
        let proof_bytes = &proof_blob[i + 8 + B_BYTES..i + 8 + B_BYTES + form_size];
        let Ok(proof) = deserialize_form(d, proof_bytes, strict) else {
            return false;
        };

        let out_y = if skip_check {
            let r = fast_pow(segment_iters, &b);
            let f1 = fast_pow_form_nucomp(&proof, d, &b, &l);
            let f2 = fast_pow_form_nucomp(x, d, &r, &l);
            let mut y = nucomp(&f1, &f2, d, &l);
            reduce(&mut y);
            y
        } else {
            let Ok(y) = verify_weso_segment(d, x, &proof, &b, segment_iters) else {
                return false;
            };
            y
        };

        *x = out_y;

        if segment_iters > *iterations {
            return false;
        }
        *iterations -= segment_iters;
    }
    true
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
    if !is_discriminant_in_range(d) {
        return false;
    }

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

    let strict = false;
    let Ok(mut x) = deserialize_form(d, x_s, strict) else {
        return false;
    };

    let mut remaining_iters = iterations;
    if !check_proof_of_time_n_wesolowski_common(
        d,
        &mut x,
        proof_blob,
        &mut remaining_iters,
        base_len,
        false,
    ) {
        return false;
    }

    let Ok(y) = deserialize_form(d, &proof_blob[..form_size], strict) else {
        return false;
    };
    let Ok(proof) = deserialize_form(d, &proof_blob[form_size..2 * form_size], strict) else {
        return false;
    };

    verify_wesolowski_proof(d, &x, &y, &proof, remaining_iters)
}

/// Create discriminant from `seed` and verify an N-Wesolowski proof
/// (chiavdf `CreateDiscriminantAndCheckProofOfTimeNWesolowski`).
pub fn create_discriminant_and_check_proof_of_time_n_wesolowski(
    seed: &[u8],
    disc_size_bits: usize,
    x_s: &[u8],
    proof_blob: &[u8],
    iterations: u64,
    depth: u64,
) -> bool {
    if !is_disc_size_bits_in_range(disc_size_bits) {
        return false;
    }
    if seed.is_empty() || !disc_size_bits.is_multiple_of(8) {
        return false;
    }
    let d = crate::discriminant::create_discriminant(seed, disc_size_bits);
    if !is_discriminant_in_range(&d) {
        return false;
    }
    check_proof_of_time_n_wesolowski(&d, x_s, proof_blob, iterations, depth)
}

/// Verify N-Wesolowski proof where final B is supplied (chiavdf
/// `CheckProofOfTimeNWesolowskiWithB`).
///
/// `proof_blob` is `[proof_form][segments...]` (no leading y form).
/// On success returns `(true, serialized_y)`; on failure `(false, [])`.
pub fn check_proof_of_time_n_wesolowski_with_b(
    d: &Integer,
    b: &Integer,
    x_s: &[u8],
    proof_blob: &[u8],
    iterations: u64,
    depth: u64,
) -> (bool, Vec<u8>) {
    if !is_discriminant_in_range(d) {
        return (false, Vec::new());
    }

    let form_size = BQFC_FORM_SIZE;
    let segment_len = 8 + B_BYTES + form_size;
    let max_depth = (usize::MAX - form_size) as u64 / segment_len as u64;
    if depth > max_depth {
        return (false, Vec::new());
    }
    let expected_len = form_size + depth as usize * segment_len;
    if proof_blob.len() != expected_len {
        return (false, Vec::new());
    }

    let strict = false;
    let Ok(mut x) = deserialize_form(d, x_s, strict) else {
        return (false, Vec::new());
    };

    let mut remaining_iters = iterations;
    if !check_proof_of_time_n_wesolowski_common(
        d,
        &mut x,
        proof_blob,
        &mut remaining_iters,
        form_size,
        false,
    ) {
        return (false, Vec::new());
    }

    let Ok(proof) = deserialize_form(d, &proof_blob[..form_size], strict) else {
        return (false, Vec::new());
    };

    let Ok(mut y_result) = verify_weso_segment(d, &x, &proof, b, remaining_iters) else {
        return (false, Vec::new());
    };

    let d_bits = num_bits(d);
    let result = serialize_form(&mut y_result, d_bits);
    (true, result)
}

/// Extract B from an N-Wesolowski proof (chiavdf `GetBFromProof`).
///
/// `proof_blob` is the full `[y][proof][segments...]` blob.
pub fn get_b_from_proof(
    d: &Integer,
    x_s: &[u8],
    proof_blob: &[u8],
    iterations: u64,
    depth: u64,
) -> Result<Integer, String> {
    if !is_discriminant_in_range(d) {
        return Err("invalid discriminant".to_string());
    }

    let form_size = BQFC_FORM_SIZE;
    let segment_len = 8 + B_BYTES + form_size;
    let base_len = 2 * form_size;
    let max_depth = (usize::MAX - base_len) as u64 / segment_len as u64;
    if depth > max_depth {
        return Err("invalid proof depth".to_string());
    }
    let expected_len = base_len + depth as usize * segment_len;
    if proof_blob.len() != expected_len {
        return Err("invalid proof length".to_string());
    }

    let strict = false;
    let Ok(mut x) = deserialize_form(d, x_s, strict) else {
        return Err("invalid input form".to_string());
    };

    let mut remaining_iters = iterations;
    if !check_proof_of_time_n_wesolowski_common(
        d,
        &mut x,
        proof_blob,
        &mut remaining_iters,
        base_len,
        true,
    ) {
        return Err("invalid proof segments".to_string());
    }

    let Ok(mut y) = deserialize_form(d, &proof_blob[..form_size], strict) else {
        return Err("invalid y form".to_string());
    };

    Ok(get_b(d, &mut x, &mut y))
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

    #[test]
    fn test_disc_size_range() {
        assert!(is_disc_size_bits_in_range(1024));
        assert!(!is_disc_size_bits_in_range(0));
        assert!(!is_disc_size_bits_in_range(BQFC_MAX_D_BITS + 1));
    }
}
