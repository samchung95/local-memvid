use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

/// Options for `beginBatchSync()`.
#[napi(object)]
pub struct JsBatchOptions {
    /// Compression level (0 = none, 1 = fast, 3 = default, 11 = max).
    pub compression_level: Option<i32>,
    /// Disable auto-checkpoint during batch (default: true).
    pub disable_auto_checkpoint: Option<bool>,
    /// Skip fsync for maximum speed — NOT crash-safe (default: false).
    pub skip_sync: Option<bool>,
    /// Pre-allocate WAL to this many bytes (0 = no pre-sizing).
    pub wal_pre_size_bytes: Option<i64>,
}

fn to_put_many_opts(opts: JsBatchOptions) -> memvid_core::PutManyOpts {
    let mut o = memvid_core::PutManyOpts::default();
    if let Some(v) = opts.compression_level {
        o.compression_level = v;
    }
    if let Some(v) = opts.disable_auto_checkpoint {
        o.disable_auto_checkpoint = v;
    }
    if let Some(v) = opts.skip_sync {
        o.skip_sync = v;
    }
    if let Some(v) = opts.wal_pre_size_bytes {
        o.wal_pre_size_bytes = v as u64;
    }
    o
}

#[napi]
impl JsMemvid {
    // -- putBytes -------------------------------------------------------------

    /// Append raw bytes as a document frame (synchronous).
    /// Returns the frame ID as a bigint.
    #[napi(js_name = "putBytesSync")]
    pub fn put_bytes_sync(&self, data: Buffer) -> napi::Result<i64> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let id = mv.put_bytes(&data).map_err(from_memvid_error)?;
        Ok(id as i64)
    }

    /// Append raw bytes as a document frame (async).
    /// Returns a Promise resolving to the frame ID as a bigint.
    #[napi(js_name = "putBytes")]
    pub async fn put_bytes_async(&self, data: Buffer) -> napi::Result<i64> {
        let inner = self.inner.clone();
        let buf: Vec<u8> = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let id = mv.put_bytes(&buf).map_err(from_memvid_error)?;
            Ok(id as i64)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- commit ---------------------------------------------------------------

    /// Flush WAL to disk, rebuilding all indexes (synchronous).
    #[napi(js_name = "commitSync")]
    pub fn commit_sync(&self) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.commit().map_err(from_memvid_error)
    }

    /// Flush WAL to disk, rebuilding all indexes (async).
    #[napi(js_name = "commit")]
    pub async fn commit_async(&self) -> napi::Result<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.commit().map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- commitSkipIndexes ----------------------------------------------------

    /// Commit pending records without rebuilding indexes (synchronous).
    /// Use for bulk ingestion; call `finalizeIndexesSync()` after all batches.
    #[napi(js_name = "commitSkipIndexesSync")]
    pub fn commit_skip_indexes_sync(&self) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.commit_skip_indexes().map_err(from_memvid_error)
    }

    /// Commit pending records without rebuilding indexes (async).
    #[napi(js_name = "commitSkipIndexes")]
    pub async fn commit_skip_indexes_async(&self) -> napi::Result<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.commit_skip_indexes().map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- finalizeIndexes ------------------------------------------------------

    /// Rebuild all indexes after bulk ingestion (synchronous).
    /// Call after one or more `commitSkipIndexesSync()` calls.
    #[napi(js_name = "finalizeIndexesSync")]
    pub fn finalize_indexes_sync(&self) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.finalize_indexes().map_err(from_memvid_error)
    }

    /// Rebuild all indexes after bulk ingestion (async).
    #[napi(js_name = "finalizeIndexes")]
    pub async fn finalize_indexes_async(&self) -> napi::Result<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.finalize_indexes().map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- batch mode -----------------------------------------------------------

    /// Enter batch mode for high-throughput ingestion (synchronous).
    #[napi(js_name = "beginBatchSync")]
    pub fn begin_batch_sync(&self, opts: JsBatchOptions) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.begin_batch(to_put_many_opts(opts))
            .map_err(from_memvid_error)
    }

    /// End batch mode and flush the WAL (synchronous).
    #[napi(js_name = "endBatchSync")]
    pub fn end_batch_sync(&self) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.end_batch().map_err(from_memvid_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_options_defaults() {
        let opts = JsBatchOptions {
            compression_level: None,
            disable_auto_checkpoint: None,
            skip_sync: None,
            wal_pre_size_bytes: None,
        };
        let converted = to_put_many_opts(opts);
        let default = memvid_core::PutManyOpts::default();
        assert_eq!(converted.compression_level, default.compression_level);
        assert_eq!(
            converted.disable_auto_checkpoint,
            default.disable_auto_checkpoint
        );
        assert_eq!(converted.skip_sync, default.skip_sync);
        assert_eq!(converted.wal_pre_size_bytes, default.wal_pre_size_bytes);
    }

    #[test]
    fn batch_options_overrides() {
        let opts = JsBatchOptions {
            compression_level: Some(11),
            disable_auto_checkpoint: Some(false),
            skip_sync: Some(true),
            wal_pre_size_bytes: Some(1024 * 1024),
        };
        let converted = to_put_many_opts(opts);
        assert_eq!(converted.compression_level, 11);
        assert!(!converted.disable_auto_checkpoint);
        assert!(converted.skip_sync);
        assert_eq!(converted.wal_pre_size_bytes, 1024 * 1024);
    }

    #[test]
    fn put_bytes_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let data = Buffer::from(vec![1, 2, 3]);
        let result = js.put_bytes_sync(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }

    #[test]
    fn commit_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.commit_sync();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }
}
