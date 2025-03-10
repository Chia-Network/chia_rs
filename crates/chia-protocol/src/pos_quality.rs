use chia_traits::chia_error::Result;

// The actual space in bytes of a plot, is _expected_plot_size(k) * UI_ACTUAL_SPACE_CONSTANT_FACTO
// This is not used in consensus, only for display purposes

pub const UI_ACTUAL_SPACE_CONSTANT_FACTOR: f32 = 0.78;

pub fn expected_plot_size(k: u32) -> Result<u64> {
    // """
    // Given the plot size parameter k (which is between 32 and 59), computes the
    // expected size of the plot in bytes (times a constant factor). This is based on efficient encoding
    // of the plot, and aims to be scale agnostic, so larger plots don't
    // necessarily get more rewards per byte. The +1 is added to give half a bit more space per entry, which
    // is necessary to store the entries in the plot.
    // """

    Ok((2 * k as u64 + 1) * (1_u64 << (k - 1)))
}

// TODO: Update this when new plot format releases
#[cfg(feature = "py-bindings")]
#[pyo3::pyfunction]
#[pyo3(name = "expected_plot_size")]
pub fn py_expected_plot_size(k: u32) -> pyo3::PyResult<u64> {
    // """
    // Given the plot size parameter k (which is between 32 and 59), computes the
    // expected size of the plot in bytes (times a constant factor). This is based on efficient encoding
    // of the plot, and aims to be scale agnostic, so larger plots don't
    // necessarily get more rewards per byte. The +1 is added to give half a bit more space per entry, which
    // is necessary to store the entries in the plot.
    // """

    Ok(expected_plot_size(k)?)
}
