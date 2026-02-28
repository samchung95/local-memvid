use pyo3::prelude::*;

use memvid_core::types::common::FrameRole;
use memvid_core::types::options::{PutManyOpts, PutOptions};

use crate::error;
use crate::lifecycle::{PyMemvid, guard_memvid};

/// Parse a Python string into a `FrameRole` via serde.
fn parse_role(role: &str) -> Result<FrameRole, String> {
    serde_json::from_value(serde_json::Value::String(role.to_string()))
        .map_err(|e| format!("invalid role '{}': {}", role, e))
}

#[pymethods]
impl PyMemvid {
    /// Append raw bytes as a document frame and return the frame ID.
    ///
    /// Both `bytes` and `bytearray` are accepted.
    fn put_bytes(&self, py: Python<'_>, data: &[u8]) -> PyResult<u64> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.put_bytes(data)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Append raw bytes with options and return the frame ID.
    ///
    /// Keyword arguments map to `PutOptions` fields. All are optional.
    #[pyo3(signature = (data, *, timestamp=None, track=None, kind=None, uri=None, title=None, tags=None, labels=None, search_text=None, enable_embedding=false, auto_tag=true, dedup=false, role=None, source_path=None, no_raw=false))]
    #[allow(clippy::too_many_arguments)]
    fn put_bytes_with_options(
        &self,
        py: Python<'_>,
        data: &[u8],
        timestamp: Option<i64>,
        track: Option<String>,
        kind: Option<String>,
        uri: Option<String>,
        title: Option<String>,
        tags: Option<Vec<String>>,
        labels: Option<Vec<String>>,
        search_text: Option<String>,
        enable_embedding: bool,
        auto_tag: bool,
        dedup: bool,
        role: Option<&str>,
        source_path: Option<String>,
        no_raw: bool,
    ) -> PyResult<u64> {
        error::catch_panic(py, || {
            let mut builder = PutOptions::builder()
                .enable_embedding(enable_embedding)
                .auto_tag(auto_tag)
                .dedup(dedup)
                .no_raw(no_raw);

            if let Some(ts) = timestamp {
                builder = builder.timestamp(ts);
            }
            if let Some(t) = track {
                builder = builder.track(t);
            }
            if let Some(k) = kind {
                builder = builder.kind(k);
            }
            if let Some(u) = uri {
                builder = builder.uri(u);
            }
            if let Some(t) = title {
                builder = builder.title(t);
            }
            if let Some(st) = search_text {
                builder = builder.search_text(st);
            }
            if let Some(sp) = source_path {
                builder = builder.source_path(sp);
            }
            if let Some(r) = role {
                let fr = parse_role(r).map_err(pyo3::exceptions::PyValueError::new_err)?;
                builder = builder.role(fr);
            }

            let mut opts = builder.build();

            if let Some(t) = tags {
                opts.tags = t;
            }
            if let Some(l) = labels {
                opts.labels = l;
            }

            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.put_bytes_with_options(data, opts)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Append raw bytes with a pre-computed embedding and return the frame ID.
    fn put_with_embedding(
        &self,
        py: Python<'_>,
        data: &[u8],
        embedding: Vec<f32>,
    ) -> PyResult<u64> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.put_with_embedding(data, embedding)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Delete a frame by ID and return the tombstone frame ID.
    fn delete_frame(&self, py: Python<'_>, frame_id: u64) -> PyResult<u64> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.delete_frame(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Enter batch mode with the given options.
    ///
    /// While in batch mode, per-entry fsync is disabled for better throughput.
    /// Call `end_batch()` to flush and restore normal mode.
    #[pyo3(signature = (*, compression_level=3, disable_auto_checkpoint=true, skip_sync=false, wal_pre_size_bytes=0))]
    fn begin_batch(
        &self,
        py: Python<'_>,
        compression_level: i32,
        disable_auto_checkpoint: bool,
        skip_sync: bool,
        wal_pre_size_bytes: u64,
    ) -> PyResult<()> {
        error::catch_panic(py, || {
            let opts = PutManyOpts {
                compression_level,
                disable_auto_checkpoint,
                skip_sync,
                wal_pre_size_bytes,
                ..Default::default()
            };
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.begin_batch(opts)
                .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Exit batch mode, flushing WAL with a single fsync.
    fn end_batch(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.end_batch().map_err(|e| error::from_memvid_error(py, e))
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
