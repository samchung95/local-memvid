use std::collections::HashMap;

use napi_derive::napi;

use memvid_core::{CanonicalEncoding, Frame, FrameId, FrameRole};

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// JsFrame
// ---------------------------------------------------------------------------

/// A frame's metadata, returned by `frameById` / `frameByUri`.
#[napi(object)]
pub struct JsFrame {
    /// Frame ID (bigint).
    pub id: i64,
    /// Unix timestamp (seconds).
    pub timestamp: i64,
    /// Content kind label (e.g. "text/plain").
    pub kind: Option<String>,
    /// Track identifier (e.g. "chat", "docs").
    pub track: Option<String>,
    /// Frame URI.
    pub uri: Option<String>,
    /// Human-readable title.
    pub title: Option<String>,
    /// User-defined tags.
    pub tags: Vec<String>,
    /// Classification labels.
    pub labels: Vec<String>,
    /// Custom key-value pairs.
    pub extra_metadata: HashMap<String, String>,
    /// Frame role: "document", "document_chunk", or "extracted_image".
    pub role: String,
    /// Canonical encoding: "plain" or "zstd".
    pub canonical_encoding: String,
    /// Indexed search text.
    pub search_text: Option<String>,
}

/// Convert a `FrameRole` to its JS string representation.
fn role_to_string(role: FrameRole) -> String {
    match role {
        FrameRole::Document => "document".to_string(),
        FrameRole::DocumentChunk => "document_chunk".to_string(),
        FrameRole::ExtractedImage => "extracted_image".to_string(),
    }
}

/// Convert a `CanonicalEncoding` to its JS string representation.
fn encoding_to_string(enc: CanonicalEncoding) -> String {
    match enc {
        CanonicalEncoding::Plain => "plain".to_string(),
        CanonicalEncoding::Zstd => "zstd".to_string(),
    }
}

/// Convert a Rust `Frame` into the JS representation.
fn from_frame(f: Frame) -> JsFrame {
    JsFrame {
        id: f.id as i64,
        timestamp: f.timestamp,
        kind: f.kind,
        track: f.track,
        uri: f.uri,
        title: f.title,
        tags: f.tags,
        labels: f.labels,
        extra_metadata: f.extra_metadata.into_iter().collect(),
        role: role_to_string(f.role),
        canonical_encoding: encoding_to_string(f.canonical_encoding),
        search_text: f.search_text,
    }
}

// ---------------------------------------------------------------------------
// JsMemvid frame access methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Retrieve a frame by its numeric ID (synchronous).
    #[napi(js_name = "frameByIdSync")]
    pub fn frame_by_id_sync(&self, frame_id: i64) -> napi::Result<JsFrame> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let frame = mv
            .frame_by_id(frame_id as FrameId)
            .map_err(from_memvid_error)?;
        Ok(from_frame(frame))
    }

    /// Retrieve a frame by its numeric ID (async).
    /// Returns a Promise resolving to `JsFrame`.
    #[napi(js_name = "frameById")]
    pub async fn frame_by_id_async(&self, frame_id: i64) -> napi::Result<JsFrame> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let frame = mv
                .frame_by_id(frame_id as FrameId)
                .map_err(from_memvid_error)?;
            Ok(from_frame(frame))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Retrieve a frame by its URI (synchronous).
    #[napi(js_name = "frameByUriSync")]
    pub fn frame_by_uri_sync(&self, uri: String) -> napi::Result<JsFrame> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let frame = mv.frame_by_uri(&uri).map_err(from_memvid_error)?;
        Ok(from_frame(frame))
    }

    /// Retrieve a frame by its URI (async).
    /// Returns a Promise resolving to `JsFrame`.
    #[napi(js_name = "frameByUri")]
    pub async fn frame_by_uri_async(&self, uri: String) -> napi::Result<JsFrame> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let frame = mv.frame_by_uri(&uri).map_err(from_memvid_error)?;
            Ok(from_frame(frame))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Total number of frames in the store.
    #[napi(getter, js_name = "frameCount")]
    pub fn frame_count(&self) -> napi::Result<u32> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.frame_count() as u32)
    }

    /// The next frame ID that will be assigned (bigint).
    #[napi(getter, js_name = "nextFrameId")]
    pub fn next_frame_id(&self) -> napi::Result<i64> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.next_frame_id() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memvid_core::FrameStatus;

    #[test]
    fn role_to_string_variants() {
        assert_eq!(role_to_string(FrameRole::Document), "document");
        assert_eq!(role_to_string(FrameRole::DocumentChunk), "document_chunk");
        assert_eq!(
            role_to_string(FrameRole::ExtractedImage),
            "extracted_image"
        );
    }

    #[test]
    fn encoding_to_string_variants() {
        assert_eq!(encoding_to_string(CanonicalEncoding::Plain), "plain");
        assert_eq!(encoding_to_string(CanonicalEncoding::Zstd), "zstd");
    }

    #[test]
    fn from_frame_conversion() {
        use std::collections::BTreeMap;

        let mut extra = BTreeMap::new();
        extra.insert("key1".to_string(), "val1".to_string());
        extra.insert("key2".to_string(), "val2".to_string());

        let frame = Frame {
            id: 42,
            timestamp: 1700000000,
            anchor_ts: None,
            anchor_source: None,
            kind: Some("text/plain".to_string()),
            track: Some("docs".to_string()),
            payload_offset: 0,
            payload_length: 100,
            checksum: [0u8; 32],
            uri: Some("mem://docs/42".to_string()),
            title: Some("Test Frame".to_string()),
            canonical_encoding: CanonicalEncoding::Zstd,
            canonical_length: Some(80),
            metadata: None,
            search_text: Some("hello world".to_string()),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            labels: vec!["label1".to_string()],
            extra_metadata: extra,
            content_dates: vec![],
            chunk_manifest: None,
            role: FrameRole::Document,
            parent_id: None,
            chunk_index: None,
            chunk_count: None,
            status: FrameStatus::Active,
            supersedes: None,
            superseded_by: None,
            source_sha256: None,
            source_path: None,
            enrichment_state: Default::default(),
        };

        let js = from_frame(frame);
        assert_eq!(js.id, 42);
        assert_eq!(js.timestamp, 1700000000);
        assert_eq!(js.kind.as_deref(), Some("text/plain"));
        assert_eq!(js.track.as_deref(), Some("docs"));
        assert_eq!(js.uri.as_deref(), Some("mem://docs/42"));
        assert_eq!(js.title.as_deref(), Some("Test Frame"));
        assert_eq!(js.tags, vec!["tag1", "tag2"]);
        assert_eq!(js.labels, vec!["label1"]);
        assert_eq!(js.extra_metadata.get("key1").map(|s| s.as_str()), Some("val1"));
        assert_eq!(js.extra_metadata.get("key2").map(|s| s.as_str()), Some("val2"));
        assert_eq!(js.extra_metadata.len(), 2);
        assert_eq!(js.role, "document");
        assert_eq!(js.canonical_encoding, "zstd");
        assert_eq!(js.search_text.as_deref(), Some("hello world"));
    }

    #[test]
    fn from_frame_minimal() {
        let frame = Frame {
            id: 0,
            timestamp: 0,
            anchor_ts: None,
            anchor_source: None,
            kind: None,
            track: None,
            payload_offset: 0,
            payload_length: 0,
            checksum: [0u8; 32],
            uri: None,
            title: None,
            canonical_encoding: CanonicalEncoding::Plain,
            canonical_length: None,
            metadata: None,
            search_text: None,
            tags: vec![],
            labels: vec![],
            extra_metadata: Default::default(),
            content_dates: vec![],
            chunk_manifest: None,
            role: FrameRole::default(),
            parent_id: None,
            chunk_index: None,
            chunk_count: None,
            status: FrameStatus::default(),
            supersedes: None,
            superseded_by: None,
            source_sha256: None,
            source_path: None,
            enrichment_state: Default::default(),
        };

        let js = from_frame(frame);
        assert_eq!(js.id, 0);
        assert!(js.kind.is_none());
        assert!(js.track.is_none());
        assert!(js.uri.is_none());
        assert!(js.title.is_none());
        assert!(js.tags.is_empty());
        assert!(js.labels.is_empty());
        assert!(js.extra_metadata.is_empty());
        assert_eq!(js.role, "document");
        assert_eq!(js.canonical_encoding, "plain");
        assert!(js.search_text.is_none());
    }

    #[test]
    fn frame_by_id_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.frame_by_id_sync(0);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn frame_by_uri_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.frame_by_uri_sync("test://uri".to_string());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn frame_count_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.frame_count();
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn next_frame_id_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.next_frame_id();
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
