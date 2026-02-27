use pyo3::prelude::*;

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

#[pymethods]
impl PyMemvid {
    /// Append raw bytes as a document frame and return the frame ID.
    ///
    /// Both `bytes` and `bytearray` are accepted.
    fn put_bytes(&self, py: Python<'_>, data: &[u8]) -> PyResult<u64> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.put_bytes(data).map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Flush pending WAL records to disk, rebuilding indexes.
    fn commit(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.commit().map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Commit pending WAL records without rebuilding any indexes.
    ///
    /// Optimized for bulk ingestion — call `finalize_indexes()` after all
    /// batches are written.
    fn commit_skip_indexes(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.commit_skip_indexes()
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Rebuild all indexes (time, Tantivy, vec) and persist the TOC.
    ///
    /// Use after bulk ingestion with `commit_skip_indexes()`.
    fn finalize_indexes(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.finalize_indexes()
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }
}
