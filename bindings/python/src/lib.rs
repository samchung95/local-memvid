use pyo3::prelude::*;

pub mod error;
pub mod lifecycle;
pub mod search;
pub mod timeline;
pub mod write;

/// Returns the memvid-core version string.
#[pyfunction]
fn version() -> String {
    memvid_core::MEMVID_CORE_VERSION.to_string()
}

/// Python module for memvid.
#[pymodule]
fn memvid(m: &Bound<'_, PyModule>) -> PyResult<()> {
    error::register(m)?;
    lifecycle::register(m)?;
    search::register(m)?;
    timeline::register(m)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
