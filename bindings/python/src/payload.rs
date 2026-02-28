use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error;
use crate::lifecycle::{PyMemvid, guard_memvid};

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// Context for a frame relative to a query.
#[pyclass(name = "FrameContext", frozen)]
pub struct PyFrameContext {
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub match_count: usize,
}

#[pymethods]
impl PyFrameContext {
    fn __repr__(&self) -> String {
        format!(
            "FrameContext(match_count={}, text={:?})",
            self.match_count,
            if self.text.len() > 60 {
                format!("{}...", &self.text[..60])
            } else {
                self.text.clone()
            }
        )
    }
}

// ---------------------------------------------------------------------------
// Payload/text methods on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Return the canonical (decompressed) payload bytes for a frame.
    fn frame_canonical_payload<'py>(
        &self,
        py: Python<'py>,
        frame_id: u64,
    ) -> PyResult<Bound<'py, PyBytes>> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let bytes = mv
                .frame_canonical_payload(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyBytes::new(py, &bytes))
        })
    }

    /// Return the full text content of a frame.
    fn frame_text_by_id(&self, py: Python<'_>, frame_id: u64) -> PyResult<String> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.frame_text_by_id(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Return a truncated preview of a frame's content.
    fn frame_preview_by_id(&self, py: Python<'_>, frame_id: u64) -> PyResult<String> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.frame_preview_by_id(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Return the embedding vector for a frame, or None if not available.
    fn frame_embedding(&self, py: Python<'_>, frame_id: u64) -> PyResult<Option<Vec<f32>>> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.frame_embedding(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Return context for a frame relative to a query string.
    fn frame_context(
        &self,
        py: Python<'_>,
        frame_id: u64,
        query: &str,
    ) -> PyResult<PyFrameContext> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let (text, match_count) = mv
                .frame_context(frame_id, query)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyFrameContext { text, match_count })
        })
    }
}

/// Register payload-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFrameContext>()?;
    Ok(())
}
