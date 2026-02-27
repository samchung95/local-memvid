use pyo3::prelude::*;

use memvid_core::types::TimelineEntry;

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};
use std::num::NonZeroU64;

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// A single timeline entry.
#[pyclass(name = "TimelineEntry", frozen)]
pub struct PyTimelineEntry {
    #[pyo3(get)]
    pub frame_id: u64,
    #[pyo3(get)]
    pub timestamp: i64,
    #[pyo3(get)]
    pub preview: String,
    #[pyo3(get)]
    pub uri: Option<String>,
    #[pyo3(get)]
    pub child_frames: Vec<u64>,
}

#[pymethods]
impl PyTimelineEntry {
    fn __repr__(&self) -> String {
        format!(
            "TimelineEntry(frame_id={}, timestamp={})",
            self.frame_id, self.timestamp
        )
    }
}

impl From<TimelineEntry> for PyTimelineEntry {
    fn from(e: TimelineEntry) -> Self {
        Self {
            frame_id: e.frame_id,
            timestamp: e.timestamp,
            preview: e.preview,
            uri: e.uri,
            child_frames: e.child_frames,
        }
    }
}

// ---------------------------------------------------------------------------
// timeline() method on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Scan frames chronologically.
    ///
    /// Returns a list of `TimelineEntry` objects.
    #[pyo3(signature = (*, limit=None, since=None, until=None, reverse=false))]
    fn timeline(
        &self,
        py: Python<'_>,
        limit: Option<u64>,
        since: Option<i64>,
        until: Option<i64>,
        reverse: bool,
    ) -> PyResult<Vec<PyTimelineEntry>> {
        error::catch_panic(py, || {
            let query = memvid_core::types::TimelineQuery {
                limit: limit.and_then(NonZeroU64::new),
                since,
                until,
                reverse,
                #[cfg(feature = "temporal_track")]
                temporal: None,
            };
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let entries = mv
                .timeline(query)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(entries.into_iter().map(PyTimelineEntry::from).collect())
        })
    }
}

/// Register timeline-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTimelineEntry>()?;
    Ok(())
}
