use napi_derive::napi;

use memvid_core::types::ask::AskContextFragmentKind;
use memvid_core::{AskMode, AskRequest, AskRetriever, VecEmbedder};

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};
use crate::search::JsSearchResponse;

// ---------------------------------------------------------------------------
// JsAskRequest
// ---------------------------------------------------------------------------

/// Ask/RAG request options.
///
/// Only `question` is required — all other fields are optional and default to
/// sensible values matching the Rust core.
#[napi(object)]
pub struct JsAskRequest {
    /// The question to ask.
    pub question: String,
    /// Maximum number of retrieval hits (default: 5).
    pub top_k: Option<u32>,
    /// Characters of context to capture around matches (default: 200).
    pub snippet_chars: Option<u32>,
    /// Restrict retrieval to a specific URI.
    pub uri: Option<String>,
    /// Restrict retrieval to a named scope/collection.
    pub scope: Option<String>,
    /// If true, return context fragments without generating an LLM answer.
    pub context_only: Option<bool>,
    /// Retrieval mode: "lex", "sem", or "hybrid" (default: "hybrid").
    pub mode: Option<String>,
}

/// Parse a mode string into the Rust `AskMode`.
fn parse_ask_mode(s: &str) -> AskMode {
    match s.to_lowercase().as_str() {
        "lex" => AskMode::Lex,
        "sem" | "semantic" => AskMode::Sem,
        "hybrid" => AskMode::Hybrid,
        _ => AskMode::default(),
    }
}

/// Convert a JS ask request into the Rust `AskRequest`.
fn to_ask_request(req: JsAskRequest) -> AskRequest {
    AskRequest {
        question: req.question,
        top_k: req.top_k.unwrap_or(5) as usize,
        snippet_chars: req.snippet_chars.unwrap_or(200) as usize,
        uri: req.uri,
        scope: req.scope,
        cursor: None,
        start: None,
        end: None,
        #[cfg(feature = "temporal_track")]
        temporal: None,
        context_only: req.context_only.unwrap_or(false),
        mode: req.mode.as_deref().map(parse_ask_mode).unwrap_or_default(),
        as_of_frame: None,
        as_of_ts: None,
        adaptive: None,
        acl_context: None,
        acl_enforcement_mode: Default::default(),
    }
}

// ---------------------------------------------------------------------------
// JsAskStats
// ---------------------------------------------------------------------------

/// Timing statistics for the ask operation.
#[napi(object)]
pub struct JsAskStats {
    /// Milliseconds spent retrieving context.
    pub retrieval_ms: f64,
    /// Milliseconds spent synthesizing the answer.
    pub synthesis_ms: f64,
    /// End-to-end latency in milliseconds.
    pub latency_ms: f64,
}

// ---------------------------------------------------------------------------
// JsAskCitation
// ---------------------------------------------------------------------------

/// A citation pointing back into the memory.
#[napi(object)]
pub struct JsAskCitation {
    /// Citation index (1-based in rendered text).
    pub index: u32,
    /// Frame ID of the cited content.
    pub frame_id: i64,
    /// URI of the cited frame.
    pub uri: String,
    /// Byte range within the chunk (start, end), if available.
    pub chunk_range_start: Option<u32>,
    /// Byte range within the chunk (end), if available.
    pub chunk_range_end: Option<u32>,
    /// Relevance score (if available).
    pub score: Option<f64>,
}

/// Convert a Rust `AskCitation` into the JS representation.
fn from_ask_citation(c: memvid_core::AskCitation) -> JsAskCitation {
    let (chunk_range_start, chunk_range_end) = match c.chunk_range {
        Some((start, end)) => (Some(start as u32), Some(end as u32)),
        None => (None, None),
    };
    JsAskCitation {
        index: c.index as u32,
        frame_id: c.frame_id as i64,
        uri: c.uri,
        chunk_range_start,
        chunk_range_end,
        score: c.score.map(|s| s as f64),
    }
}

// ---------------------------------------------------------------------------
// JsAskContextFragment
// ---------------------------------------------------------------------------

/// A fragment of retrieval context sent to the synthesizer.
#[napi(object)]
pub struct JsAskContextFragment {
    /// 1-based rank within the result set.
    pub rank: u32,
    /// Frame ID of the source content.
    pub frame_id: i64,
    /// URI of the source frame.
    pub uri: String,
    /// Title of the source frame (if available).
    pub title: Option<String>,
    /// Relevance score (if available).
    pub score: Option<f64>,
    /// Number of query term matches.
    pub match_count: u32,
    /// Text of this context fragment.
    pub text: String,
    /// Fragment kind: "full" or "summary" (if available).
    pub kind: Option<String>,
}

/// Convert a Rust `AskContextFragment` into the JS representation.
fn from_ask_context_fragment(f: memvid_core::types::ask::AskContextFragment) -> JsAskContextFragment {
    JsAskContextFragment {
        rank: f.rank as u32,
        frame_id: f.frame_id as i64,
        uri: f.uri,
        title: f.title,
        score: f.score.map(|s| s as f64),
        match_count: f.matches as u32,
        text: f.text,
        kind: f.kind.map(|k| match k {
            AskContextFragmentKind::Full => "full".to_string(),
            AskContextFragmentKind::Summary => "summary".to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// JsAskResponse
// ---------------------------------------------------------------------------

/// Full response from an ask/RAG operation.
#[napi(object)]
pub struct JsAskResponse {
    /// The original question echoed back.
    pub question: String,
    /// Retrieval mode used: "lex", "sem", or "hybrid".
    pub mode: String,
    /// Actual retriever used: "lex", "semantic", "hybrid", "lex_fallback", or "timeline_fallback".
    pub retriever: String,
    /// Whether this was a context-only request.
    pub context_only: bool,
    /// The underlying search response with hits and context.
    pub retrieval: JsSearchResponse,
    /// The synthesized answer (if context_only was false and a synthesizer was available).
    pub answer: Option<String>,
    /// Structured citations within the answer.
    pub citations: Vec<JsAskCitation>,
    /// Context fragments used for synthesis.
    pub context_fragments: Vec<JsAskContextFragment>,
    /// Timing statistics.
    pub stats: JsAskStats,
}

/// Convert `AskMode` to a JS string.
fn ask_mode_to_string(mode: &AskMode) -> String {
    match mode {
        AskMode::Lex => "lex".to_string(),
        AskMode::Sem => "sem".to_string(),
        AskMode::Hybrid => "hybrid".to_string(),
    }
}

/// Convert `AskRetriever` to a JS string.
fn ask_retriever_to_string(r: &AskRetriever) -> String {
    match r {
        AskRetriever::Lex => "lex".to_string(),
        AskRetriever::Semantic => "semantic".to_string(),
        AskRetriever::Hybrid => "hybrid".to_string(),
        AskRetriever::LexFallback => "lex_fallback".to_string(),
        AskRetriever::TimelineFallback => "timeline_fallback".to_string(),
    }
}

/// Convert a Rust `SearchResponse` into `JsSearchResponse` (re-uses logic from search module).
fn search_response_to_js(
    resp: memvid_core::types::search::SearchResponse,
) -> JsSearchResponse {
    use memvid_core::types::search::SearchEngineKind;
    let engine = match &resp.engine {
        SearchEngineKind::Tantivy => "tantivy".to_string(),
        SearchEngineKind::LexFallback => "lex_fallback".to_string(),
        SearchEngineKind::Hybrid => "hybrid".to_string(),
    };
    JsSearchResponse {
        query: resp.query,
        elapsed_ms: resp.elapsed_ms as f64,
        total_hits: resp.total_hits as u32,
        hits: resp
            .hits
            .into_iter()
            .map(|hit| crate::search::JsSearchHit {
                rank: hit.rank as u32,
                frame_id: hit.frame_id as i64,
                uri: hit.uri,
                title: hit.title,
                text: hit.text,
                score: hit.score.map(|s| s as f64),
                chunk_text: hit.chunk_text,
                match_count: hit.matches as u32,
            })
            .collect(),
        context: resp.context,
        next_cursor: resp.next_cursor,
        engine,
    }
}

/// Convert a Rust `AskResponse` into the JS representation.
fn from_ask_response(resp: memvid_core::AskResponse) -> JsAskResponse {
    JsAskResponse {
        question: resp.question,
        mode: ask_mode_to_string(&resp.mode),
        retriever: ask_retriever_to_string(&resp.retriever),
        context_only: resp.context_only,
        retrieval: search_response_to_js(resp.retrieval),
        answer: resp.answer,
        citations: resp.citations.into_iter().map(from_ask_citation).collect(),
        context_fragments: resp
            .context_fragments
            .into_iter()
            .map(from_ask_context_fragment)
            .collect(),
        stats: JsAskStats {
            retrieval_ms: resp.stats.retrieval_ms as f64,
            synthesis_ms: resp.stats.synthesis_ms as f64,
            latency_ms: resp.stats.latency_ms as f64,
        },
    }
}

// ---------------------------------------------------------------------------
// Dummy VecEmbedder for type parameter
// ---------------------------------------------------------------------------

/// Placeholder embedder — never actually called, only used to satisfy
/// the `Option<&E>` generic on `Memvid::ask()` when passing `None`.
struct NoEmbedder;

impl VecEmbedder for NoEmbedder {
    fn embed_query(&self, _text: &str) -> memvid_core::Result<Vec<f32>> {
        Err(memvid_core::MemvidError::VecNotEnabled)
    }
    fn embedding_dimension(&self) -> usize {
        0
    }
}

// ---------------------------------------------------------------------------
// JsMemvid ask methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Run a retrieval-augmented query (synchronous).
    ///
    /// Returns context fragments and optionally an LLM-synthesized answer.
    /// Set `contextOnly: true` to skip answer synthesis and return only
    /// retrieval context.
    #[napi(js_name = "askSync")]
    pub fn ask_sync(&self, request: JsAskRequest) -> napi::Result<JsAskResponse> {
        let req = to_ask_request(request);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let resp = mv
            .ask(req, None::<&NoEmbedder>)
            .map_err(from_memvid_error)?;
        Ok(from_ask_response(resp))
    }

    /// Run a retrieval-augmented query (async).
    ///
    /// Returns a Promise resolving to a `JsAskResponse`.
    #[napi(js_name = "ask")]
    pub async fn ask_async(&self, request: JsAskRequest) -> napi::Result<JsAskResponse> {
        let req = to_ask_request(request);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            let resp = mv
                .ask(req, None::<&NoEmbedder>)
                .map_err(from_memvid_error)?;
            Ok(from_ask_response(resp))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memvid_core::types::ask::AskContextFragment;

    #[test]
    fn ask_request_defaults() {
        let req = JsAskRequest {
            question: "What is memvid?".to_string(),
            top_k: None,
            snippet_chars: None,
            uri: None,
            scope: None,
            context_only: None,
            mode: None,
        };
        let converted = to_ask_request(req);
        assert_eq!(converted.question, "What is memvid?");
        assert_eq!(converted.top_k, 5);
        assert_eq!(converted.snippet_chars, 200);
        assert!(converted.uri.is_none());
        assert!(converted.scope.is_none());
        assert!(!converted.context_only);
        assert_eq!(converted.mode, AskMode::Hybrid);
    }

    #[test]
    fn ask_request_overrides() {
        let req = JsAskRequest {
            question: "Tell me about X".to_string(),
            top_k: Some(20),
            snippet_chars: Some(500),
            uri: Some("mem://docs".to_string()),
            scope: Some("project".to_string()),
            context_only: Some(true),
            mode: Some("lex".to_string()),
        };
        let converted = to_ask_request(req);
        assert_eq!(converted.question, "Tell me about X");
        assert_eq!(converted.top_k, 20);
        assert_eq!(converted.snippet_chars, 500);
        assert_eq!(converted.uri.as_deref(), Some("mem://docs"));
        assert_eq!(converted.scope.as_deref(), Some("project"));
        assert!(converted.context_only);
        assert_eq!(converted.mode, AskMode::Lex);
    }

    #[test]
    fn ask_mode_parsing() {
        assert_eq!(parse_ask_mode("lex"), AskMode::Lex);
        assert_eq!(parse_ask_mode("LEX"), AskMode::Lex);
        assert_eq!(parse_ask_mode("sem"), AskMode::Sem);
        assert_eq!(parse_ask_mode("semantic"), AskMode::Sem);
        assert_eq!(parse_ask_mode("hybrid"), AskMode::Hybrid);
        assert_eq!(parse_ask_mode("unknown"), AskMode::Hybrid); // default
    }

    #[test]
    fn ask_mode_to_string_roundtrip() {
        assert_eq!(ask_mode_to_string(&AskMode::Lex), "lex");
        assert_eq!(ask_mode_to_string(&AskMode::Sem), "sem");
        assert_eq!(ask_mode_to_string(&AskMode::Hybrid), "hybrid");
    }

    #[test]
    fn ask_retriever_to_string_all_variants() {
        assert_eq!(ask_retriever_to_string(&AskRetriever::Lex), "lex");
        assert_eq!(ask_retriever_to_string(&AskRetriever::Semantic), "semantic");
        assert_eq!(ask_retriever_to_string(&AskRetriever::Hybrid), "hybrid");
        assert_eq!(
            ask_retriever_to_string(&AskRetriever::LexFallback),
            "lex_fallback"
        );
        assert_eq!(
            ask_retriever_to_string(&AskRetriever::TimelineFallback),
            "timeline_fallback"
        );
    }

    #[test]
    fn ask_citation_conversion() {
        let citation = memvid_core::AskCitation {
            index: 1,
            frame_id: 42,
            uri: "mem://test".to_string(),
            chunk_range: Some((10, 50)),
            score: Some(0.85),
        };
        let js = from_ask_citation(citation);
        assert_eq!(js.index, 1);
        assert_eq!(js.frame_id, 42);
        assert_eq!(js.uri, "mem://test");
        assert_eq!(js.chunk_range_start, Some(10));
        assert_eq!(js.chunk_range_end, Some(50));
        assert!((js.score.unwrap() - 0.85).abs() < 0.001);
    }

    #[test]
    fn ask_citation_conversion_no_chunk_range() {
        let citation = memvid_core::AskCitation {
            index: 0,
            frame_id: 99,
            uri: "mem://other".to_string(),
            chunk_range: None,
            score: None,
        };
        let js = from_ask_citation(citation);
        assert_eq!(js.index, 0);
        assert_eq!(js.frame_id, 99);
        assert!(js.chunk_range_start.is_none());
        assert!(js.chunk_range_end.is_none());
        assert!(js.score.is_none());
    }

    #[test]
    fn ask_context_fragment_conversion() {
        let frag = AskContextFragment {
            rank: 1,
            frame_id: 7,
            uri: "mem://frag".to_string(),
            title: Some("Fragment Title".to_string()),
            score: Some(0.92),
            matches: 5,
            range: Some((0, 100)),
            chunk_range: None,
            text: "Some context text".to_string(),
            kind: Some(AskContextFragmentKind::Full),
            #[cfg(feature = "temporal_track")]
            temporal: None,
        };
        let js = from_ask_context_fragment(frag);
        assert_eq!(js.rank, 1);
        assert_eq!(js.frame_id, 7);
        assert_eq!(js.uri, "mem://frag");
        assert_eq!(js.title.as_deref(), Some("Fragment Title"));
        assert!((js.score.unwrap() - 0.92).abs() < 0.001);
        assert_eq!(js.match_count, 5);
        assert_eq!(js.text, "Some context text");
        assert_eq!(js.kind.as_deref(), Some("full"));
    }

    #[test]
    fn ask_context_fragment_summary_kind() {
        let frag = AskContextFragment {
            rank: 2,
            frame_id: 8,
            uri: "mem://sum".to_string(),
            title: None,
            score: None,
            matches: 0,
            range: None,
            chunk_range: None,
            text: "".to_string(),
            kind: Some(AskContextFragmentKind::Summary),
            #[cfg(feature = "temporal_track")]
            temporal: None,
        };
        let js = from_ask_context_fragment(frag);
        assert_eq!(js.kind.as_deref(), Some("summary"));
    }

    #[test]
    fn ask_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let req = JsAskRequest {
            question: "hello".to_string(),
            top_k: None,
            snippet_chars: None,
            uri: None,
            scope: None,
            context_only: None,
            mode: None,
        };
        let result = js.ask_sync(req);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
