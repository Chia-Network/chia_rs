use pyo3::prelude::*;

/// Create a discriminant from a seed and bit length.
/// Returns a hex string representation of the (negative) discriminant,
/// matching the chiavdf format: e.g. "-3abc...".
/// Callers convert via: int(result, 16)
#[pyfunction]
fn create_discriminant(seed: &[u8], length: usize) -> String {
    let d = crate::discriminant::create_discriminant(seed, length);
    // d is negative; format magnitude as hex with leading '-'
    format!("-{:x}", d.magnitude())
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
    disc: &str,
    input_el: &[u8],
    output: &[u8],
    number_of_iterations: u64,
    _discriminant_size: usize,
    witness_type: u64,
) -> bool {
    let d: num_bigint::BigInt = match disc.parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    crate::verifier::check_proof_of_time_n_wesolowski(
        &d,
        input_el,
        output,
        number_of_iterations,
        witness_type,
    )
}

#[pymodule]
fn chia_vdf_verify(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(create_discriminant, m)?)?;
    m.add_function(wrap_pyfunction!(verify_n_wesolowski, m)?)?;
    Ok(())
}
