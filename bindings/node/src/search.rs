use napi_derive::napi;

use memvid_core::types::search::{SearchEngineKind, SearchRequest};

use crate::error::from_memvid_error;
use crate::memvid::{JsMemvid, guard_memvid, lock_inner};

// ---------------------------------------------------------------------------
// JsSearchRequest
// ---------------------------------------------------------------------------

/// Search request options.
///
/// Only `query` is required — all other fields are optional and default to
/// sensible values matching the Rust core.
#[napi(object)]
pub struct JsSearchRequest {
    /// The search query string.
    pub query: String,
    /// Maximum number of hits to return (default: 10).
    pub top_k: Option<u32>,
    /// Characters of context to capture around matches (default: 200).
    pub snippet_chars: Option<u32>,
    /// Restrict search to a specific URI.
    pub uri: Option<String>,
    /// Restrict search to a named scope/collection.
    pub scope: Option<String>,
    /// Pagination cursor from a previous search response.
    pub cursor: Option<String>,
    /// Disable sketch pre-filtering for this query (default: false).
    pub no_sketch: Option<bool>,
}

/// Convert a JS search request into the Rust `SearchRequest`.
fn to_search_request(req: JsSearchRequest) -> SearchRequest {
    SearchRequest {
        query: req.query,
        top_k: req.top_k.unwrap_or(10) as usize,
        snippet_chars: req.snippet_chars.unwrap_or(200) as usize,
        uri: req.uri,
        scope: req.scope,
        cursor: req.cursor,
        #[cfg(feature = "temporal_track")]
        temporal: None,
        as_of_frame: None,
        as_of_ts: None,
        no_sketch: req.no_sketch.unwrap_or(false),
        acl_context: None,
        acl_enforcement_mode: Default::default(),
    }
}

// ---------------------------------------------------------------------------
// JsSearchHit
// ---------------------------------------------------------------------------

/// A single ranked search hit.
#[napi(object)]
pub struct JsSearchHit {
    /// 1-based rank within the result set.
    pub rank: u32,
    /// Frame ID (bigint).
    pub frame_id: i64,
    /// URI of the matched frame.
    pub uri: String,
    /// Optional title of the matched frame.
    pub title: Option<String>,
    /// Snippet text around the match.
    pub text: String,
    /// Relevance score (if available).
    pub score: Option<f64>,
    /// Chunk text (if the hit is within a chunk).
    pub chunk_text: Option<String>,
    /// Number of query term matches in this hit.
    pub match_count: u32,
}

/// Convert a Rust `SearchHit` into the JS representation.
fn from_search_hit(hit: memvid_core::types::search::SearchHit) -> JsSearchHit {
    JsSearchHit {
        rank: hit.rank as u32,
        frame_id: hit.frame_id as i64,
        uri: hit.uri,
        title: hit.title,
        text: hit.text,
        score: hit.score.map(|s| s as f64),
        chunk_text: hit.chunk_text,
        match_count: hit.matches as u32,
    }
}

// ---------------------------------------------------------------------------
// JsSearchResponse
// ---------------------------------------------------------------------------

/// Full search response with hits, timing, and pagination.
#[napi(object)]
pub struct JsSearchResponse {
    /// The query string echoed back.
    pub query: String,
    /// Milliseconds spent on the search.
    pub elapsed_ms: f64,
    /// Total number of hits found (before pagination).
    pub total_hits: u32,
    /// Ranked search hits for this page.
    pub hits: Vec<JsSearchHit>,
    /// Concatenated context snippets.
    pub context: String,
    /// Cursor for fetching the next page (if more results exist).
    pub next_cursor: Option<String>,
    /// Search engine used: "tantivy", "lex_fallback", or "hybrid".
    pub engine: String,
}

/// Convert engine kind enum to a JS string.
fn engine_kind_to_string(kind: &SearchEngineKind) -> String {
    match kind {
        SearchEngineKind::Tantivy => "tantivy".to_string(),
        SearchEngineKind::LexFallback => "lex_fallback".to_string(),
        SearchEngineKind::Hybrid => "hybrid".to_string(),
    }
}

/// Convert a Rust `SearchResponse` into the JS representation.
fn from_search_response(resp: memvid_core::types::search::SearchResponse) -> JsSearchResponse {
    JsSearchResponse {
        query: resp.query,
        elapsed_ms: resp.elapsed_ms as f64,
        total_hits: resp.total_hits as u32,
        hits: resp.hits.into_iter().map(from_search_hit).collect(),
        context: resp.context,
        next_cursor: resp.next_cursor,
        engine: engine_kind_to_string(&resp.engine),
    }
}

// ---------------------------------------------------------------------------
// JsMemvid search methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Search the memvid file (synchronous).
    #[napi(js_name = "searchSync")]
    pub fn search_sync(&self, request: JsSearchRequest) -> napi::Result<JsSearchResponse> {
        let req = to_search_request(request);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let resp = mv.search(req).map_err(from_memvid_error)?;
        Ok(from_search_response(resp))
    }

    /// Search the memvid file (async).
    /// Returns a Promise resolving to a `JsSearchResponse`.
    #[napi(js_name = "search")]
    pub async fn search_async(&self, request: JsSearchRequest) -> napi::Result<JsSearchResponse> {
        let req = to_search_request(request);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let resp = mv.search(req).map_err(from_memvid_error)?;
            Ok(from_search_response(resp))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_defaults() {
        let req = JsSearchRequest {
            query: "hello".to_string(),
            top_k: None,
            snippet_chars: None,
            uri: None,
            scope: None,
            cursor: None,
            no_sketch: None,
        };
        let converted = to_search_request(req);
        assert_eq!(converted.query, "hello");
        assert_eq!(converted.top_k, 10);
        assert_eq!(converted.snippet_chars, 200);
        assert!(converted.uri.is_none());
        assert!(converted.scope.is_none());
        assert!(converted.cursor.is_none());
        assert!(!converted.no_sketch);
    }

    #[test]
    fn search_request_overrides() {
        let req = JsSearchRequest {
            query: "test query".to_string(),
            top_k: Some(50),
            snippet_chars: Some(500),
            uri: Some("mem://docs".to_string()),
            scope: Some("project".to_string()),
            cursor: Some("abc123".to_string()),
            no_sketch: Some(true),
        };
        let converted = to_search_request(req);
        assert_eq!(converted.query, "test query");
        assert_eq!(converted.top_k, 50);
        assert_eq!(converted.snippet_chars, 500);
        assert_eq!(converted.uri.as_deref(), Some("mem://docs"));
        assert_eq!(converted.scope.as_deref(), Some("project"));
        assert_eq!(converted.cursor.as_deref(), Some("abc123"));
        assert!(converted.no_sketch);
    }

    #[test]
    fn search_hit_conversion() {
        let hit = memvid_core::types::search::SearchHit {
            rank: 1,
            frame_id: 42,
            uri: "mem://test".to_string(),
            title: Some("Test Title".to_string()),
            range: (0, 100),
            text: "matched text here".to_string(),
            matches: 3,
            chunk_range: None,
            chunk_text: Some("chunk content".to_string()),
            score: Some(0.95),
            metadata: None,
        };
        let js_hit = from_search_hit(hit);
        assert_eq!(js_hit.rank, 1);
        assert_eq!(js_hit.frame_id, 42);
        assert_eq!(js_hit.uri, "mem://test");
        assert_eq!(js_hit.title.as_deref(), Some("Test Title"));
        assert_eq!(js_hit.text, "matched text here");
        assert_eq!(js_hit.match_count, 3);
        assert_eq!(js_hit.chunk_text.as_deref(), Some("chunk content"));
        assert!((js_hit.score.unwrap() - 0.95).abs() < 0.001);
    }

    #[test]
    fn search_response_conversion() {
        let resp = memvid_core::types::search::SearchResponse {
            query: "test".to_string(),
            elapsed_ms: 42,
            total_hits: 100,
            params: memvid_core::types::search::SearchParams {
                top_k: 10,
                snippet_chars: 200,
                cursor: None,
            },
            hits: vec![],
            context: "context text".to_string(),
            next_cursor: Some("next_page".to_string()),
            engine: SearchEngineKind::Tantivy,
        };
        let js_resp = from_search_response(resp);
        assert_eq!(js_resp.query, "test");
        assert!((js_resp.elapsed_ms - 42.0).abs() < 0.001);
        assert_eq!(js_resp.total_hits, 100);
        assert!(js_resp.hits.is_empty());
        assert_eq!(js_resp.context, "context text");
        assert_eq!(js_resp.next_cursor.as_deref(), Some("next_page"));
        assert_eq!(js_resp.engine, "tantivy");
    }

    #[test]
    fn engine_kind_strings() {
        assert_eq!(engine_kind_to_string(&SearchEngineKind::Tantivy), "tantivy");
        assert_eq!(
            engine_kind_to_string(&SearchEngineKind::LexFallback),
            "lex_fallback"
        );
        assert_eq!(engine_kind_to_string(&SearchEngineKind::Hybrid), "hybrid");
    }

    #[test]
    fn search_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let req = JsSearchRequest {
            query: "hello".to_string(),
            top_k: None,
            snippet_chars: None,
            uri: None,
            scope: None,
            cursor: None,
            no_sketch: None,
        };
        let result = js.search_sync(req);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
