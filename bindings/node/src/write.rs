use napi::bindgen_prelude::*;
use napi_derive::napi;

use memvid_core::FrameRole;

use crate::error::from_memvid_error;
use crate::memvid::{JsMemvid, guard_memvid, lock_inner};

// ---------------------------------------------------------------------------
// JsPutOptions
// ---------------------------------------------------------------------------

/// Options for writing frames with metadata.
///
/// All fields are optional — omitted fields use sensible defaults matching the
/// Rust `PutOptions::default()`.
#[napi(object)]
pub struct JsPutOptions {
    /// Unix timestamp in seconds. If omitted, uses current time.
    pub timestamp: Option<i64>,
    /// Logical track name (e.g. "chat", "docs").
    pub track: Option<String>,
    /// MIME-like kind hint (e.g. "text/plain").
    pub kind: Option<String>,
    /// Unique URI for deduplication / retrieval.
    pub uri: Option<String>,
    /// Human-readable title.
    pub title: Option<String>,
    /// Tags for categorisation.
    pub tags: Option<Vec<String>>,
    /// Labels for filtering.
    pub labels: Option<Vec<String>>,
    /// Pre-extracted search text (bypasses extraction).
    pub search_text: Option<String>,
    /// Generate an embedding for this frame.
    pub enable_embedding: Option<bool>,
    /// Auto-tag from content (default: true).
    pub auto_tag: Option<bool>,
    /// Skip ingestion if BLAKE3 hash matches an existing frame.
    pub dedup: Option<bool>,
    /// Frame role: "document" (default), "document_chunk", or "extracted_image".
    pub role: Option<String>,
    /// Original source file path (for no-raw reference tracking).
    pub source_path: Option<String>,
    /// Don't store raw binary content, only extracted text + hash.
    pub no_raw: Option<bool>,
}

/// Convert a JS options object into the Rust `PutOptions`.
fn to_put_options(opts: JsPutOptions) -> napi::Result<memvid_core::PutOptions> {
    let mut o = memvid_core::PutOptions::default();
    if let Some(v) = opts.timestamp {
        o.timestamp = Some(v);
    }
    o.track = opts.track;
    o.kind = opts.kind;
    o.uri = opts.uri;
    o.title = opts.title;
    if let Some(v) = opts.tags {
        o.tags = v;
    }
    if let Some(v) = opts.labels {
        o.labels = v;
    }
    o.search_text = opts.search_text;
    if let Some(v) = opts.enable_embedding {
        o.enable_embedding = v;
    }
    if let Some(v) = opts.auto_tag {
        o.auto_tag = v;
    }
    if let Some(v) = opts.dedup {
        o.dedup = v;
    }
    if let Some(role_str) = opts.role {
        o.role = parse_frame_role(&role_str)?;
    }
    o.source_path = opts.source_path;
    if let Some(v) = opts.no_raw {
        o.no_raw = v;
    }
    Ok(o)
}

/// Parse a JS string into a [`FrameRole`].
fn parse_frame_role(s: &str) -> napi::Result<FrameRole> {
    match s {
        "document" => Ok(FrameRole::Document),
        "document_chunk" => Ok(FrameRole::DocumentChunk),
        "extracted_image" => Ok(FrameRole::ExtractedImage),
        other => Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!(
                "Invalid role '{other}': expected 'document', 'document_chunk', or 'extracted_image'"
            ),
        )),
    }
}

// ---------------------------------------------------------------------------
// JsBatchOptions
// ---------------------------------------------------------------------------

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

    // -- putBytesWithOptions --------------------------------------------------

    /// Append raw bytes with metadata options (synchronous).
    /// Returns the frame ID as a bigint.
    #[napi(js_name = "putBytesWithOptionsSync")]
    pub fn put_bytes_with_options_sync(
        &self,
        data: Buffer,
        options: JsPutOptions,
    ) -> napi::Result<i64> {
        let opts = to_put_options(options)?;
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let id = mv
            .put_bytes_with_options(&data, opts)
            .map_err(from_memvid_error)?;
        Ok(id as i64)
    }

    /// Append raw bytes with metadata options (async).
    /// Returns a Promise resolving to the frame ID as a bigint.
    #[napi(js_name = "putBytesWithOptions")]
    pub async fn put_bytes_with_options_async(
        &self,
        data: Buffer,
        options: JsPutOptions,
    ) -> napi::Result<i64> {
        let opts = to_put_options(options)?;
        let inner = self.inner.clone();
        let buf: Vec<u8> = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let id = mv
                .put_bytes_with_options(&buf, opts)
                .map_err(from_memvid_error)?;
            Ok(id as i64)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- putWithEmbedding -----------------------------------------------------

    /// Append raw bytes with a pre-computed embedding vector (synchronous).
    /// Returns the frame ID as a bigint.
    #[napi(js_name = "putWithEmbeddingSync")]
    pub fn put_with_embedding_sync(
        &self,
        data: Buffer,
        embedding: Float32Array,
    ) -> napi::Result<i64> {
        let emb: Vec<f32> = embedding.to_vec();
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let id = mv
            .put_with_embedding(&data, emb)
            .map_err(from_memvid_error)?;
        Ok(id as i64)
    }

    /// Append raw bytes with a pre-computed embedding vector (async).
    /// Returns a Promise resolving to the frame ID as a bigint.
    #[napi(js_name = "putWithEmbedding")]
    pub async fn put_with_embedding_async(
        &self,
        data: Buffer,
        embedding: Float32Array,
    ) -> napi::Result<i64> {
        let emb: Vec<f32> = embedding.to_vec();
        let inner = self.inner.clone();
        let buf: Vec<u8> = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let id = mv
                .put_with_embedding(&buf, emb)
                .map_err(from_memvid_error)?;
            Ok(id as i64)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- deleteFrame ----------------------------------------------------------

    /// Delete a frame by ID, creating a tombstone (synchronous).
    /// Returns the tombstone frame ID as a bigint.
    #[napi(js_name = "deleteFrameSync")]
    pub fn delete_frame_sync(&self, frame_id: i64) -> napi::Result<i64> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let tombstone_id = mv
            .delete_frame(frame_id as u64)
            .map_err(from_memvid_error)?;
        Ok(tombstone_id as i64)
    }

    /// Delete a frame by ID, creating a tombstone (async).
    /// Returns a Promise resolving to the tombstone frame ID as a bigint.
    #[napi(js_name = "deleteFrame")]
    pub async fn delete_frame_async(&self, frame_id: i64) -> napi::Result<i64> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let tombstone_id = mv
                .delete_frame(frame_id as u64)
                .map_err(from_memvid_error)?;
            Ok(tombstone_id as i64)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
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

    // -- US-005 tests ---------------------------------------------------------

    #[test]
    fn put_options_defaults() {
        let opts = JsPutOptions {
            timestamp: None,
            track: None,
            kind: None,
            uri: None,
            title: None,
            tags: None,
            labels: None,
            search_text: None,
            enable_embedding: None,
            auto_tag: None,
            dedup: None,
            role: None,
            source_path: None,
            no_raw: None,
        };
        let converted = to_put_options(opts).unwrap();
        let default = memvid_core::PutOptions::default();
        assert_eq!(converted.timestamp, default.timestamp);
        assert_eq!(converted.track, default.track);
        assert_eq!(converted.kind, default.kind);
        assert_eq!(converted.uri, default.uri);
        assert_eq!(converted.title, default.title);
        assert_eq!(converted.tags, default.tags);
        assert_eq!(converted.labels, default.labels);
        assert_eq!(converted.search_text, default.search_text);
        assert_eq!(converted.enable_embedding, default.enable_embedding);
        assert_eq!(converted.auto_tag, default.auto_tag);
        assert_eq!(converted.dedup, default.dedup);
        assert_eq!(converted.role, default.role);
        assert_eq!(converted.source_path, default.source_path);
        assert_eq!(converted.no_raw, default.no_raw);
    }

    #[test]
    fn put_options_overrides() {
        let opts = JsPutOptions {
            timestamp: Some(1234567890),
            track: Some("chat".to_string()),
            kind: Some("text/plain".to_string()),
            uri: Some("mem://test".to_string()),
            title: Some("Test Doc".to_string()),
            tags: Some(vec!["a".to_string(), "b".to_string()]),
            labels: Some(vec!["important".to_string()]),
            search_text: Some("hello world".to_string()),
            enable_embedding: Some(true),
            auto_tag: Some(false),
            dedup: Some(true),
            role: Some("document_chunk".to_string()),
            source_path: Some("/tmp/test.txt".to_string()),
            no_raw: Some(true),
        };
        let converted = to_put_options(opts).unwrap();
        assert_eq!(converted.timestamp, Some(1234567890));
        assert_eq!(converted.track.as_deref(), Some("chat"));
        assert_eq!(converted.kind.as_deref(), Some("text/plain"));
        assert_eq!(converted.uri.as_deref(), Some("mem://test"));
        assert_eq!(converted.title.as_deref(), Some("Test Doc"));
        assert_eq!(converted.tags, vec!["a", "b"]);
        assert_eq!(converted.labels, vec!["important"]);
        assert_eq!(converted.search_text.as_deref(), Some("hello world"));
        assert!(converted.enable_embedding);
        assert!(!converted.auto_tag);
        assert!(converted.dedup);
        assert_eq!(converted.role, FrameRole::DocumentChunk);
        assert_eq!(converted.source_path.as_deref(), Some("/tmp/test.txt"));
        assert!(converted.no_raw);
    }

    #[test]
    fn parse_frame_role_valid() {
        assert_eq!(parse_frame_role("document").unwrap(), FrameRole::Document);
        assert_eq!(
            parse_frame_role("document_chunk").unwrap(),
            FrameRole::DocumentChunk
        );
        assert_eq!(
            parse_frame_role("extracted_image").unwrap(),
            FrameRole::ExtractedImage
        );
    }

    #[test]
    fn parse_frame_role_invalid() {
        let result = parse_frame_role("invalid");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid role"));
    }

    #[test]
    fn put_bytes_with_options_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let data = Buffer::from(vec![1, 2, 3]);
        let opts = JsPutOptions {
            timestamp: None,
            track: None,
            kind: None,
            uri: None,
            title: None,
            tags: None,
            labels: None,
            search_text: None,
            enable_embedding: None,
            auto_tag: None,
            dedup: None,
            role: None,
            source_path: None,
            no_raw: None,
        };
        let result = js.put_bytes_with_options_sync(data, opts);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }

    #[test]
    fn delete_frame_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.delete_frame_sync(42);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }
}
