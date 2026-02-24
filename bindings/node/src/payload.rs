use napi::bindgen_prelude::*;
use napi_derive::napi;

use memvid_core::FrameId;

use crate::error::from_memvid_error;
use crate::memvid::{JsMemvid, guard_memvid, lock_inner};

// ---------------------------------------------------------------------------
// JsFrameContext — result of frameContext()
// ---------------------------------------------------------------------------

/// Result of a contextual text extraction for a frame.
#[napi(object)]
pub struct JsFrameContext {
    /// The contextual text extracted from the frame.
    pub text: String,
    /// Number of query matches found.
    pub match_count: u32,
}

// ---------------------------------------------------------------------------
// JsMemvid payload / text retrieval methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Read the canonical (raw/decompressed) payload of a frame (synchronous).
    /// Returns a Buffer containing the frame's bytes.
    #[napi(js_name = "frameCanonicalPayloadSync")]
    pub fn frame_canonical_payload_sync(&self, frame_id: i64) -> napi::Result<Buffer> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let bytes = mv
            .frame_canonical_payload(frame_id as FrameId)
            .map_err(from_memvid_error)?;
        Ok(bytes.into())
    }

    /// Read the canonical (raw/decompressed) payload of a frame (async).
    /// Returns a Promise resolving to a Buffer.
    #[napi(js_name = "frameCanonicalPayload")]
    pub async fn frame_canonical_payload_async(&self, frame_id: i64) -> napi::Result<Buffer> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let bytes = mv
                .frame_canonical_payload(frame_id as FrameId)
                .map_err(from_memvid_error)?;
            Ok(bytes.into())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Get the full text content of a frame (synchronous).
    /// Returns the complete text suitable for LLM processing.
    #[napi(js_name = "frameTextByIdSync")]
    pub fn frame_text_by_id_sync(&self, frame_id: i64) -> napi::Result<String> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.frame_text_by_id(frame_id as FrameId)
            .map_err(from_memvid_error)
    }

    /// Get the full text content of a frame (async).
    /// Returns a Promise resolving to a string.
    #[napi(js_name = "frameTextById")]
    pub async fn frame_text_by_id_async(&self, frame_id: i64) -> napi::Result<String> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.frame_text_by_id(frame_id as FrameId)
                .map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Get a truncated preview of a frame's content (synchronous).
    #[napi(js_name = "framePreviewByIdSync")]
    pub fn frame_preview_by_id_sync(&self, frame_id: i64) -> napi::Result<String> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        mv.frame_preview_by_id(frame_id as FrameId)
            .map_err(from_memvid_error)
    }

    /// Get a truncated preview of a frame's content (async).
    /// Returns a Promise resolving to a string.
    #[napi(js_name = "framePreviewById")]
    pub async fn frame_preview_by_id_async(&self, frame_id: i64) -> napi::Result<String> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            mv.frame_preview_by_id(frame_id as FrameId)
                .map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Get the embedding vector for a frame (synchronous).
    /// Returns a Float32Array or null if no embedding exists.
    #[napi(js_name = "frameEmbeddingSync")]
    pub fn frame_embedding_sync(&self, frame_id: i64) -> napi::Result<Option<Float32Array>> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let emb = mv
            .frame_embedding(frame_id as FrameId)
            .map_err(from_memvid_error)?;
        Ok(emb.map(Float32Array::new))
    }

    /// Get the embedding vector for a frame (async).
    /// Returns a Promise resolving to Float32Array or null.
    #[napi(js_name = "frameEmbedding")]
    pub async fn frame_embedding_async(&self, frame_id: i64) -> napi::Result<Option<Float32Array>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let emb = mv
                .frame_embedding(frame_id as FrameId)
                .map_err(from_memvid_error)?;
            Ok(emb.map(Float32Array::new))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Extract contextual text from a frame for a given query (synchronous).
    /// Returns an object with `text` and `matchCount` fields.
    #[napi(js_name = "frameContextSync")]
    pub fn frame_context_sync(&self, frame_id: i64, query: String) -> napi::Result<JsFrameContext> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let (text, match_count) = mv
            .frame_context(frame_id as FrameId, &query)
            .map_err(from_memvid_error)?;
        Ok(JsFrameContext {
            text,
            match_count: match_count as u32,
        })
    }

    /// Extract contextual text from a frame for a given query (async).
    /// Returns a Promise resolving to `{ text: string, matchCount: number }`.
    #[napi(js_name = "frameContext")]
    pub async fn frame_context_async(
        &self,
        frame_id: i64,
        query: String,
    ) -> napi::Result<JsFrameContext> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let (text, match_count) = mv
                .frame_context(frame_id as FrameId, &query)
                .map_err(from_memvid_error)?;
            Ok(JsFrameContext {
                text,
                match_count: match_count as u32,
            })
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn closed_instance() -> JsMemvid {
        JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn frame_canonical_payload_sync_rejects_closed() {
        let js = closed_instance();
        let err = match js.frame_canonical_payload_sync(0) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn frame_text_by_id_sync_rejects_closed() {
        let js = closed_instance();
        let err = match js.frame_text_by_id_sync(0) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn frame_preview_by_id_sync_rejects_closed() {
        let js = closed_instance();
        let err = match js.frame_preview_by_id_sync(0) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn frame_embedding_sync_rejects_closed() {
        let js = closed_instance();
        let err = match js.frame_embedding_sync(0) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn frame_context_sync_rejects_closed() {
        let js = closed_instance();
        let err = match js.frame_context_sync(0, "test".to_string()) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn js_frame_context_fields() {
        let ctx = JsFrameContext {
            text: "hello world".to_string(),
            match_count: 3,
        };
        assert_eq!(ctx.text, "hello world");
        assert_eq!(ctx.match_count, 3);
    }
}
