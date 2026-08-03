//! Python bindings mirroring [chiavdf `_chiavdf`](https://github.com/Chia-Network/chiavdf/blob/main/src/python_bindings/fastvdf.cpp).
//!
//! Verification only — `prove` is not implemented (use chiavdf for proving).

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use crate::bqfc::{BQFC_FORM_SIZE, BQFC_MAX_D_BITS};
use crate::integer::{
    format_hex_chia, from_signed_bytes_be, parse_integer_auto, to_signed_bytes_be,
};
use crate::proof_common::deserialize_form;
use crate::verifier::{
    check_proof_of_time_n_wesolowski, check_proof_of_time_n_wesolowski_with_b,
    create_discriminant_and_check_proof_of_time_n_wesolowski, get_b_from_proof,
    is_disc_size_bits_in_range, verify_wesolowski_proof,
};

fn py_bytes_to_vec(data: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    // bytes / bytearray, or str (chiavdf historically passed forms as std::string).
    data.extract::<Vec<u8>>()
        .or_else(|_| data.extract::<&str>().map(|s| s.as_bytes().to_vec()))
        .map_err(|_| PyValueError::new_err("expected bytes-like form (bytes or str)"))
}

fn validate_disc_length(length: usize) -> PyResult<()> {
    if !is_disc_size_bits_in_range(length) || !length.is_multiple_of(8) {
        return Err(PyValueError::new_err(format!(
            "discriminant_size_bits must be a positive multiple of 8 and <= {BQFC_MAX_D_BITS}"
        )));
    }
    Ok(())
}

/// Create a discriminant from a seed and bit length.
///
/// Returns a chiavdf-style hex string (`-0x…`), suitable for `int(result, 16)`.
#[pyfunction]
fn create_discriminant(py: Python<'_>, seed: &[u8], length: usize) -> PyResult<String> {
    if seed.is_empty() {
        return Err(PyValueError::new_err("seed cannot be empty"));
    }
    validate_disc_length(length)?;
    let seed = seed.to_vec();
    Ok(py.detach(move || {
        let d = crate::discriminant::create_discriminant(&seed, length);
        format_hex_chia(&d)
    }))
}

/// Create a discriminant from a seed and bit length, returning bytes.
/// Format: `[sign_byte][magnitude_be...]`. Use with `verify_n_wesolowski_bytes`
/// to avoid repeated string parse overhead.
#[pyfunction]
fn create_discriminant_bytes(py: Python<'_>, seed: &[u8], length: usize) -> PyResult<Vec<u8>> {
    if seed.is_empty() {
        return Err(PyValueError::new_err("seed cannot be empty"));
    }
    validate_disc_length(length)?;
    let seed = seed.to_vec();
    Ok(py.detach(move || {
        let d = crate::discriminant::create_discriminant(&seed, length);
        to_signed_bytes_be(&d)
    }))
}

/// Verify a simple (depth-0) Wesolowski proof.
///
/// Mirrors chiavdf `verify_wesolowski`.
#[pyfunction]
fn verify_wesolowski(
    py: Python<'_>,
    discriminant: &str,
    x_s: &Bound<'_, PyAny>,
    y_s: &Bound<'_, PyAny>,
    proof_s: &Bound<'_, PyAny>,
    num_iterations: u64,
) -> bool {
    let Some(d) = parse_integer_auto(discriminant) else {
        return false;
    };
    let Ok(x_s) = py_bytes_to_vec(x_s) else {
        return false;
    };
    let Ok(y_s) = py_bytes_to_vec(y_s) else {
        return false;
    };
    let Ok(proof_s) = py_bytes_to_vec(proof_s) else {
        return false;
    };
    py.detach(move || {
        let Ok(x) = deserialize_form(&d, &x_s, false) else {
            return false;
        };
        let Ok(y) = deserialize_form(&d, &y_s, false) else {
            return false;
        };
        let Ok(proof) = deserialize_form(&d, &proof_s, false) else {
            return false;
        };
        verify_wesolowski_proof(&d, &x, &y, &proof, num_iterations)
    })
}

/// Verify an N-Wesolowski proof.
///
/// Arguments match chiavdf `verify_n_wesolowski`:
///   disc             — discriminant as a decimal or `0x` hex string
///   input_el         — 100-byte serialized input form
///   output           — y form + proof bytes concatenated
///   number_of_iterations
///   discriminant_size — API compat (validated for range, unused otherwise)
///   witness_type     — proof depth (0, 1, 2, …)
#[pyfunction]
fn verify_n_wesolowski(
    py: Python<'_>,
    disc: &str,
    input_el: &[u8],
    output: &[u8],
    number_of_iterations: u64,
    discriminant_size: usize,
    witness_type: u64,
) -> bool {
    if !is_disc_size_bits_in_range(discriminant_size) {
        return false;
    }
    let Some(d) = parse_integer_auto(disc) else {
        return false;
    };
    let input_el = input_el.to_vec();
    let output = output.to_vec();
    py.detach(move || {
        check_proof_of_time_n_wesolowski(&d, &input_el, &output, number_of_iterations, witness_type)
    })
}

/// Verify an N-Wesolowski proof using discriminant bytes from
/// `create_discriminant_bytes`.
#[pyfunction]
fn verify_n_wesolowski_bytes(
    py: Python<'_>,
    disc_bytes: &[u8],
    input_el: &[u8],
    output: &[u8],
    number_of_iterations: u64,
    discriminant_size: usize,
    witness_type: u64,
) -> bool {
    if !is_disc_size_bits_in_range(discriminant_size) {
        return false;
    }
    let Some(d) = from_signed_bytes_be(disc_bytes) else {
        return false;
    };
    let input_el = input_el.to_vec();
    let output = output.to_vec();
    py.detach(move || {
        check_proof_of_time_n_wesolowski(&d, &input_el, &output, number_of_iterations, witness_type)
    })
}

/// Create a discriminant from `challenge_hash` and verify an N-Wesolowski proof.
///
/// Mirrors chiavdf `create_discriminant_and_verify_n_wesolowski`.
#[pyfunction]
fn create_discriminant_and_verify_n_wesolowski(
    py: Python<'_>,
    challenge_hash: &[u8],
    discriminant_size_bits: usize,
    x_s: &Bound<'_, PyAny>,
    proof_blob: &[u8],
    num_iterations: u64,
    recursion: u64,
) -> bool {
    let Ok(x_s) = py_bytes_to_vec(x_s) else {
        return false;
    };
    let challenge_hash = challenge_hash.to_vec();
    let proof_blob = proof_blob.to_vec();
    py.detach(move || {
        create_discriminant_and_check_proof_of_time_n_wesolowski(
            &challenge_hash,
            discriminant_size_bits,
            &x_s,
            &proof_blob,
            num_iterations,
            recursion,
        )
    })
}

/// Verify an N-Wesolowski proof given an explicit final B.
///
/// Mirrors chiavdf `verify_n_wesolowski_with_b`.
/// `proof_blob` is `[proof_form][segments…]` (no leading y).
/// Returns `(is_valid, y_bytes)`.
#[pyfunction]
fn verify_n_wesolowski_with_b(
    py: Python<'_>,
    discriminant: &str,
    b: &str,
    x_s: &Bound<'_, PyAny>,
    proof_blob: &[u8],
    num_iterations: u64,
    recursion: u64,
) -> (bool, Vec<u8>) {
    let Some(d) = parse_integer_auto(discriminant) else {
        return (false, Vec::new());
    };
    let Some(b) = parse_integer_auto(b) else {
        return (false, Vec::new());
    };
    let Ok(x_s) = py_bytes_to_vec(x_s) else {
        return (false, Vec::new());
    };
    let proof_blob = proof_blob.to_vec();
    py.detach(move || {
        check_proof_of_time_n_wesolowski_with_b(
            &d,
            &b,
            &x_s,
            &proof_blob,
            num_iterations,
            recursion,
        )
    })
}

/// Extract B from an N-Wesolowski proof (chiavdf `get_b_from_n_wesolowski`).
///
/// Returns a chiavdf-style hex string (`0x…`). Raises on invalid proof.
#[pyfunction]
fn get_b_from_n_wesolowski(
    py: Python<'_>,
    discriminant: &str,
    x_s: &Bound<'_, PyAny>,
    proof_blob: &[u8],
    num_iterations: u64,
    recursion: u64,
) -> PyResult<String> {
    let d = parse_integer_auto(discriminant)
        .ok_or_else(|| PyValueError::new_err("bad discriminant"))?;
    let x_s = py_bytes_to_vec(x_s)?;
    let proof_blob = proof_blob.to_vec();
    py.detach(move || {
        get_b_from_proof(&d, &x_s, &proof_blob, num_iterations, recursion)
            .map(|b| format_hex_chia(&b))
            .map_err(PyRuntimeError::new_err)
    })
}

/// Deserialize a BQFC-compressed form, returning `(a_bytes, b_bytes)`.
///
/// `disc`: discriminant as decimal or `0x` hex string.
/// `data`: 100-byte BQFC form.
/// `strict`: if true, reject forms where `|b| > a`.
#[pyfunction]
#[pyo3(signature = (disc, data, *, strict = true))]
fn bqfc_deserialize(disc: &str, data: &[u8], strict: bool) -> PyResult<(Vec<u8>, Vec<u8>)> {
    if data.len() != BQFC_FORM_SIZE {
        return Err(PyValueError::new_err(format!(
            "expected {BQFC_FORM_SIZE}-byte form"
        )));
    }
    let d = parse_integer_auto(disc).ok_or_else(|| PyValueError::new_err("bad discriminant"))?;
    let f = deserialize_form(&d, data, strict).map_err(PyValueError::new_err)?;
    Ok((to_signed_bytes_be(&f.a), to_signed_bytes_be(&f.b)))
}

/// Register VDF verification functions on a Python module.
///
/// Used by the standalone `chia_vdf_verify` extension and by the `chia_rs` wheel.
pub fn add_to_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(create_discriminant, m)?)?;
    m.add_function(wrap_pyfunction!(create_discriminant_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(verify_wesolowski, m)?)?;
    m.add_function(wrap_pyfunction!(verify_n_wesolowski, m)?)?;
    m.add_function(wrap_pyfunction!(verify_n_wesolowski_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(
        create_discriminant_and_verify_n_wesolowski,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(verify_n_wesolowski_with_b, m)?)?;
    m.add_function(wrap_pyfunction!(get_b_from_n_wesolowski, m)?)?;
    m.add_function(wrap_pyfunction!(bqfc_deserialize, m)?)?;
    Ok(())
}

#[pymodule]
fn chia_vdf_verify(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_to_module(m)
}
