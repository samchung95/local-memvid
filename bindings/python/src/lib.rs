use pyo3::prelude::*;

pub mod error;

/// Returns the memvid-core version string.
#[pyfunction]
fn version() -> String {
    memvid_core::MEMVID_CORE_VERSION.to_string()
}

/// Python module for memvid.
#[pymodule]
fn memvid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    error::register(m)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
