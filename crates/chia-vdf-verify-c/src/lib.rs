//! C FFI mirroring [chiavdf `c_wrapper.h`](https://github.com/Chia-Network/chiavdf/blob/main/src/c_bindings/c_wrapper.h).
//!
//! Only verification is implemented. `prove_wrapper` always returns a null `ByteArray`.

#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::slice;

use chia_vdf_verify::bqfc::BQFC_FORM_SIZE;
use chia_vdf_verify::discriminant::create_discriminant;
use chia_vdf_verify::integer::{from_bytes_be, to_bytes_be_padded};
use chia_vdf_verify::verifier::check_proof_of_time_n_wesolowski;
use malachite_nz::integer::Integer;

/// Matches `ByteArray` in `include/c_wrapper.h`.
#[repr(C)]
pub struct ByteArray {
    pub data: *mut u8,
    pub length: usize,
}

/// Create a discriminant from `seed` and write `|D|` as big-endian into `result`.
///
/// `result` must point to a buffer of exactly `size_bits / 8` bytes.
/// Returns `false` on null pointers, invalid `size_bits`, or panic.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn create_discriminant_wrapper(
    seed: *const u8,
    seed_size: usize,
    size_bits: usize,
    result: *mut u8,
) -> bool {
    if seed.is_null() || result.is_null() {
        return false;
    }
    if size_bits == 0 || !size_bits.is_multiple_of(8) {
        return false;
    }
    let byte_len = size_bits / 8;

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Caller guarantees `seed` is valid for `seed_size` bytes and
        // `result` is valid for `size_bits / 8` bytes.
        let seed = unsafe { slice::from_raw_parts(seed, seed_size) };
        let d = create_discriminant(seed, size_bits);
        let bytes = to_bytes_be_padded(&d, byte_len);
        // SAFETY: as above.
        let out = unsafe { slice::from_raw_parts_mut(result, byte_len) };
        out.copy_from_slice(&bytes);
        true
    }));

    outcome.unwrap_or(false)
}

/// Proving is not implemented. Always returns `{ data: null, length: 0 }`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn prove_wrapper(
    _challenge_hash: *const u8,
    _challenge_size: usize,
    _x_s: *const u8,
    _x_s_size: usize,
    _discriminant_size_bits: usize,
    _num_iterations: u64,
) -> ByteArray {
    ByteArray {
        data: std::ptr::null_mut(),
        length: 0,
    }
}

/// Verify an n-Wesolowski proof.
///
/// `discriminant_bytes` is the unsigned big-endian magnitude of `|D|`
/// (same encoding as chiavdf). `x_s` must be a 100-byte BQFC form.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verify_n_wesolowski_wrapper(
    discriminant_bytes: *const u8,
    discriminant_size: usize,
    x_s: *const u8,
    proof_blob: *const u8,
    proof_blob_size: usize,
    num_iterations: u64,
    recursion: u64,
) -> bool {
    if discriminant_bytes.is_null() || x_s.is_null() || proof_blob.is_null() {
        return false;
    }
    if discriminant_size == 0 {
        return false;
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: Caller guarantees each pointer is valid for the given length;
        // `x_s` is always `BQFC_FORM_SIZE` bytes (chiavdf convention).
        let disc = unsafe { slice::from_raw_parts(discriminant_bytes, discriminant_size) };
        let x_s = unsafe { slice::from_raw_parts(x_s, BQFC_FORM_SIZE) };
        let proof = unsafe { slice::from_raw_parts(proof_blob, proof_blob_size) };

        let magnitude = from_bytes_be(disc);
        let d: Integer = -magnitude;
        check_proof_of_time_n_wesolowski(&d, x_s, proof, num_iterations, recursion)
    }));

    outcome.unwrap_or(false)
}

/// Free a `ByteArray` allocated by `prove_wrapper` (no-op for null data).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn delete_byte_array(array: ByteArray) {
    if array.data.is_null() || array.length == 0 {
        return;
    }
    // SAFETY: `data` was allocated by this crate (currently unused — prove
    // always returns null). Ownership length == capacity for this path.
    let _ = unsafe { Vec::from_raw_parts(array.data, array.length, array.length) };
}
