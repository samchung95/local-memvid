use pyo3::prelude::*;

pub mod ask;
pub mod blob;
pub mod error;
pub mod frame;
pub mod lifecycle;
pub mod memory;
pub mod memory_query;
pub mod mesh;
pub mod payload;
pub mod schema;
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
    ask::register(m)?;
    blob::register(m)?;
    error::register(m)?;
    frame::register(m)?;
    lifecycle::register(m)?;
    memory_query::register(m)?;
    mesh::register(m)?;
    payload::register(m)?;
    schema::register(m)?;
    search::register(m)?;
    timeline::register(m)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
