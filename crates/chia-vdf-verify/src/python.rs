use pyo3::prelude::*;

use crate::integer::from_signed_bytes_be;
use crate::verifier::check_proof_of_time_n_wesolowski;

/// Create a discriminant from a seed and bit length.
/// Returns a hex string representation of the (negative) discriminant,
/// matching the chiavdf format: e.g. "-3abc...".
/// Callers convert via: int(result, 16)
#[pyfunction]
fn create_discriminant(py: Python<'_>, seed: &[u8], length: usize) -> String {
    let seed = seed.to_vec();
    py.detach(move || {
        let d = crate::discriminant::create_discriminant(&seed, length);
        format!("{d:x}")
    })
}

/// Create a discriminant from a seed and bit length, returning bytes.
/// Format: [sign_byte][magnitude_be...]. Use with verify_n_wesolowski_bytes
/// to avoid repeated decimal string parse overhead.
#[pyfunction]
fn create_discriminant_bytes(py: Python<'_>, seed: &[u8], length: usize) -> Vec<u8> {
    let seed = seed.to_vec();
    py.detach(move || {
        let d = crate::discriminant::create_discriminant(&seed, length);
        crate::integer::to_signed_bytes_be(&d)
    })
}

/// Verify a VDF N-Wesolowski proof.
///
/// Arguments match the chiavdf.verify_n_wesolowski signature:
///   disc             - discriminant as a decimal string (negative)
///   input_el         - 100-byte serialized input form
///   output           - serialized output form + proof bytes concatenated
///   number_of_iterations - total VDF iterations
///   discriminant_size    - discriminant bit size (API compat only, unused)
///   witness_type         - proof depth (0, 1, 2, …)
#[pyfunction]
fn verify_n_wesolowski(
    py: Python<'_>,
    disc: &str,
    input_el: &[u8],
    output: &[u8],
    number_of_iterations: u64,
    _discriminant_size: usize,
    witness_type: u64,
) -> bool {
    let Ok(d) = disc.parse::<malachite_nz::integer::Integer>() else {
        return false;
    };
    let input_el = input_el.to_vec();
    let output = output.to_vec();
    py.detach(move || {
        check_proof_of_time_n_wesolowski(&d, &input_el, &output, number_of_iterations, witness_type)
    })
}

/// Verify a VDF N-Wesolowski proof using discriminant bytes.
/// Avoids the decimal string parse overhead of verify_n_wesolowski.
/// disc_bytes: output of create_discriminant_bytes (format: [sign_byte][magnitude_be...])
#[pyfunction]
fn verify_n_wesolowski_bytes(
    py: Python<'_>,
    disc_bytes: &[u8],
    input_el: &[u8],
    output: &[u8],
    number_of_iterations: u64,
    _discriminant_size: usize,
    witness_type: u64,
) -> bool {
    let Some(d) = from_signed_bytes_be(disc_bytes) else {
        return false;
    };
    let input_el = input_el.to_vec();
    let output = output.to_vec();
    py.detach(move || {
        check_proof_of_time_n_wesolowski(&d, &input_el, &output, number_of_iterations, witness_type)
    })
}

/// Deserialize a BQFC-compressed form, returning (a_bytes, b_bytes) or raising ValueError.
/// disc: discriminant as decimal string (negative)
/// data: 100-byte BQFC form
/// strict: if true, reject forms where |b| > a
#[pyfunction]
#[pyo3(signature = (disc, data, *, strict = true))]
fn bqfc_deserialize(disc: &str, data: &[u8], strict: bool) -> PyResult<(Vec<u8>, Vec<u8>)> {
    let d = disc
        .parse::<malachite_nz::integer::Integer>()
        .map_err(|()| pyo3::exceptions::PyValueError::new_err("bad discriminant"))?;
    let f = crate::proof_common::deserialize_form(&d, data, strict)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    Ok((
        crate::integer::to_signed_bytes_be(&f.a),
        crate::integer::to_signed_bytes_be(&f.b),
    ))
}

#[pymodule]
fn chia_vdf_verify(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(create_discriminant, m)?)?;
    m.add_function(wrap_pyfunction!(create_discriminant_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(verify_n_wesolowski, m)?)?;
    m.add_function(wrap_pyfunction!(verify_n_wesolowski_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(bqfc_deserialize, m)?)?;
    Ok(())
}
