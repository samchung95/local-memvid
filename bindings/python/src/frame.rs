use std::collections::HashMap;

use pyo3::prelude::*;

use memvid_core::types::common::{CanonicalEncoding, FrameRole};
use memvid_core::types::frame::Frame;

use crate::error;
use crate::lifecycle::{PyMemvid, guard_memvid};

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// Frame metadata.
#[pyclass(name = "Frame", frozen)]
pub struct PyFrame {
    #[pyo3(get)]
    pub id: u64,
    #[pyo3(get)]
    pub timestamp: i64,
    #[pyo3(get)]
    pub kind: Option<String>,
    #[pyo3(get)]
    pub track: Option<String>,
    #[pyo3(get)]
    pub uri: Option<String>,
    #[pyo3(get)]
    pub title: Option<String>,
    #[pyo3(get)]
    pub tags: Vec<String>,
    #[pyo3(get)]
    pub labels: Vec<String>,
    #[pyo3(get)]
    pub role: String,
    #[pyo3(get)]
    pub canonical_encoding: String,
    #[pyo3(get)]
    pub search_text: Option<String>,
    raw_extra_metadata: HashMap<String, String>,
}

#[pymethods]
impl PyFrame {
    /// Extra metadata as a dict.
    #[getter]
    fn extra_metadata(&self) -> HashMap<String, String> {
        self.raw_extra_metadata.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Frame(id={}, uri={:?}, kind={:?})",
            self.id, self.uri, self.kind
        )
    }
}

fn role_to_string(role: FrameRole) -> String {
    match role {
        FrameRole::Document => "document".to_string(),
        FrameRole::DocumentChunk => "document_chunk".to_string(),
        FrameRole::ExtractedImage => "extracted_image".to_string(),
    }
}

fn encoding_to_string(enc: CanonicalEncoding) -> String {
    match enc {
        CanonicalEncoding::Plain => "plain".to_string(),
        CanonicalEncoding::Zstd => "zstd".to_string(),
    }
}

impl From<Frame> for PyFrame {
    fn from(f: Frame) -> Self {
        Self {
            id: f.id,
            timestamp: f.timestamp,
            kind: f.kind,
            track: f.track,
            uri: f.uri,
            title: f.title,
            tags: f.tags,
            labels: f.labels,
            raw_extra_metadata: f.extra_metadata.into_iter().collect(),
            role: role_to_string(f.role),
            canonical_encoding: encoding_to_string(f.canonical_encoding),
            search_text: f.search_text,
        }
    }
}

// ---------------------------------------------------------------------------
// Frame access methods on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Look up a frame by its ID.
    fn frame_by_id(&self, py: Python<'_>, frame_id: u64) -> PyResult<PyFrame> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let frame = mv
                .frame_by_id(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyFrame::from(frame))
        })
    }

    /// Look up a frame by its URI.
    fn frame_by_uri(&self, py: Python<'_>, uri: &str) -> PyResult<PyFrame> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let frame = mv
                .frame_by_uri(uri)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyFrame::from(frame))
        })
    }

    /// Total number of frames.
    #[getter]
    fn frame_count(&self, py: Python<'_>) -> PyResult<usize> {
        let lock = guard_memvid!(self, py);
        Ok(lock.as_ref().unwrap().frame_count())
    }

    /// The next frame ID that would be assigned on insert.
    #[getter]
    fn next_frame_id(&self, py: Python<'_>) -> PyResult<u64> {
        let lock = guard_memvid!(self, py);
        Ok(lock.as_ref().unwrap().next_frame_id())
    }
}

/// Register frame-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFrame>()?;
    Ok(())
}
