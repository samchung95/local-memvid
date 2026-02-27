use pyo3::prelude::*;

/// Returns the memvid-core version string.
#[pyfunction]
fn version() -> String {
    memvid_core::MEMVID_CORE_VERSION.to_string()
}

/// Python module for memvid.
#[pymodule]
fn memvid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
