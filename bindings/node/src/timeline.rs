use std::num::NonZeroU64;

use napi_derive::napi;

use memvid_core::types::TimelineQuery;

use crate::error::from_memvid_error;
use crate::memvid::{JsMemvid, guard_memvid, lock_inner};

// ---------------------------------------------------------------------------
// JsTimelineQuery
// ---------------------------------------------------------------------------

/// Timeline query options.
///
/// All fields are optional — an empty object returns all frames in
/// chronological order.
#[napi(object)]
pub struct JsTimelineQuery {
    /// Maximum number of entries to return.
    pub limit: Option<i64>,
    /// Include only frames at or after this Unix timestamp (seconds).
    pub since: Option<i64>,
    /// Include only frames at or before this Unix timestamp (seconds).
    pub until: Option<i64>,
    /// Return entries in reverse-chronological order (default: false).
    pub reverse: Option<bool>,
}

/// Convert a JS timeline query into the Rust `TimelineQuery`.
fn to_timeline_query(q: JsTimelineQuery) -> TimelineQuery {
    TimelineQuery {
        limit: q
            .limit
            .and_then(|v| u64::try_from(v).ok())
            .and_then(NonZeroU64::new),
        since: q.since,
        until: q.until,
        reverse: q.reverse.unwrap_or(false),
        #[cfg(feature = "temporal_track")]
        temporal: None,
    }
}

// ---------------------------------------------------------------------------
// JsTimelineEntry
// ---------------------------------------------------------------------------

/// A single timeline entry representing a frame.
#[napi(object)]
pub struct JsTimelineEntry {
    /// Frame ID (bigint).
    pub frame_id: i64,
    /// Unix timestamp (seconds) when the frame was created.
    pub timestamp: i64,
    /// Short text preview of the frame content.
    pub preview: String,
    /// URI of the frame (if available).
    pub uri: Option<String>,
    /// IDs of child frames (e.g., extracted images).
    pub child_frames: Vec<i64>,
}

/// Convert a Rust `TimelineEntry` into the JS representation.
fn from_timeline_entry(entry: memvid_core::types::TimelineEntry) -> JsTimelineEntry {
    JsTimelineEntry {
        frame_id: entry.frame_id as i64,
        timestamp: entry.timestamp,
        preview: entry.preview,
        uri: entry.uri,
        child_frames: entry.child_frames.into_iter().map(|id| id as i64).collect(),
    }
}

// ---------------------------------------------------------------------------
// JsMemvid timeline methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Scan frames chronologically (synchronous).
    #[napi(js_name = "timelineSync")]
    pub fn timeline_sync(&self, query: JsTimelineQuery) -> napi::Result<Vec<JsTimelineEntry>> {
        let tq = to_timeline_query(query);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let entries = mv.timeline(tq).map_err(from_memvid_error)?;
        Ok(entries.into_iter().map(from_timeline_entry).collect())
    }

    /// Scan frames chronologically (async).
    /// Returns a Promise resolving to `JsTimelineEntry[]`.
    #[napi(js_name = "timeline")]
    pub async fn timeline_async(
        &self,
        query: JsTimelineQuery,
    ) -> napi::Result<Vec<JsTimelineEntry>> {
        let tq = to_timeline_query(query);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let entries = mv.timeline(tq).map_err(from_memvid_error)?;
            Ok(entries.into_iter().map(from_timeline_entry).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_query_defaults() {
        let q = JsTimelineQuery {
            limit: None,
            since: None,
            until: None,
            reverse: None,
        };
        let converted = to_timeline_query(q);
        assert!(converted.limit.is_none());
        assert!(converted.since.is_none());
        assert!(converted.until.is_none());
        assert!(!converted.reverse);
    }

    #[test]
    fn timeline_query_overrides() {
        let q = JsTimelineQuery {
            limit: Some(50),
            since: Some(1000),
            until: Some(2000),
            reverse: Some(true),
        };
        let converted = to_timeline_query(q);
        assert_eq!(converted.limit.unwrap().get(), 50);
        assert_eq!(converted.since, Some(1000));
        assert_eq!(converted.until, Some(2000));
        assert!(converted.reverse);
    }

    #[test]
    fn timeline_query_zero_limit_becomes_none() {
        let q = JsTimelineQuery {
            limit: Some(0),
            since: None,
            until: None,
            reverse: None,
        };
        let converted = to_timeline_query(q);
        // NonZeroU64::new(0) returns None
        assert!(converted.limit.is_none());
    }

    #[test]
    fn timeline_entry_conversion() {
        let entry = memvid_core::types::TimelineEntry {
            frame_id: 42,
            timestamp: 1700000000,
            preview: "Hello world".to_string(),
            uri: Some("mem://docs/1".to_string()),
            child_frames: vec![43, 44],
            #[cfg(feature = "temporal_track")]
            temporal: None,
        };
        let js = from_timeline_entry(entry);
        assert_eq!(js.frame_id, 42);
        assert_eq!(js.timestamp, 1700000000);
        assert_eq!(js.preview, "Hello world");
        assert_eq!(js.uri.as_deref(), Some("mem://docs/1"));
        assert_eq!(js.child_frames, vec![43i64, 44]);
    }

    #[test]
    fn timeline_entry_no_uri_no_children() {
        let entry = memvid_core::types::TimelineEntry {
            frame_id: 1,
            timestamp: 0,
            preview: String::new(),
            uri: None,
            child_frames: vec![],
            #[cfg(feature = "temporal_track")]
            temporal: None,
        };
        let js = from_timeline_entry(entry);
        assert_eq!(js.frame_id, 1);
        assert!(js.uri.is_none());
        assert!(js.child_frames.is_empty());
    }

    #[test]
    fn timeline_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let q = JsTimelineQuery {
            limit: None,
            since: None,
            until: None,
            reverse: None,
        };
        let result = js.timeline_sync(q);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
