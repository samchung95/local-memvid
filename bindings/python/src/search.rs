use pyo3::prelude::*;

use memvid_core::types::search::{
    SearchEngineKind, SearchHit as CoreSearchHit, SearchRequest,
    SearchResponse as CoreSearchResponse,
};

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// A single search result hit.
#[pyclass(name = "SearchHit", frozen)]
pub struct PySearchHit {
    #[pyo3(get)]
    pub rank: usize,
    #[pyo3(get)]
    pub frame_id: u64,
    #[pyo3(get)]
    pub uri: String,
    #[pyo3(get)]
    pub title: Option<String>,
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub score: Option<f32>,
    #[pyo3(get)]
    pub chunk_text: Option<String>,
    #[pyo3(get)]
    pub match_count: usize,
}

#[pymethods]
impl PySearchHit {
    fn __repr__(&self) -> String {
        format!(
            "SearchHit(rank={}, frame_id={}, score={:?})",
            self.rank, self.frame_id, self.score
        )
    }
}

/// Response from a search query.
#[pyclass(name = "SearchResponse", frozen)]
pub struct PySearchResponse {
    #[pyo3(get)]
    pub query: String,
    #[pyo3(get)]
    pub elapsed_ms: u64,
    #[pyo3(get)]
    pub total_hits: usize,
    #[pyo3(get)]
    pub context: String,
    #[pyo3(get)]
    pub next_cursor: Option<String>,
    #[pyo3(get)]
    pub engine: String,
    /// Stored internally; converted to Python list via the `hits` getter.
    raw_hits: Vec<CoreSearchHit>,
}

#[pymethods]
impl PySearchResponse {
    /// Ranked search hits.
    #[getter]
    fn hits(&self) -> Vec<PySearchHit> {
        self.raw_hits
            .iter()
            .map(|h| PySearchHit {
                rank: h.rank,
                frame_id: h.frame_id,
                uri: h.uri.clone(),
                title: h.title.clone(),
                text: h.text.clone(),
                score: h.score,
                chunk_text: h.chunk_text.clone(),
                match_count: h.matches,
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "SearchResponse(query={:?}, total_hits={}, hits={})",
            self.query,
            self.total_hits,
            self.raw_hits.len()
        )
    }
}

fn engine_to_string(engine: SearchEngineKind) -> String {
    match engine {
        SearchEngineKind::Tantivy => "tantivy".to_string(),
        SearchEngineKind::LexFallback => "lex_fallback".to_string(),
        SearchEngineKind::Hybrid => "hybrid".to_string(),
    }
}

impl From<CoreSearchResponse> for PySearchResponse {
    fn from(r: CoreSearchResponse) -> Self {
        Self {
            query: r.query,
            elapsed_ms: r.elapsed_ms as u64,
            total_hits: r.total_hits,
            raw_hits: r.hits,
            context: r.context,
            next_cursor: r.next_cursor,
            engine: engine_to_string(r.engine),
        }
    }
}

// ---------------------------------------------------------------------------
// search() method on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Search the memvid file using a text query.
    ///
    /// Returns a `SearchResponse` with ranked hits and context.
    #[pyo3(signature = (query, *, top_k=10, snippet_chars=200, uri=None, scope=None, cursor=None, no_sketch=false))]
    fn search(
        &self,
        py: Python<'_>,
        query: String,
        top_k: usize,
        snippet_chars: usize,
        uri: Option<String>,
        scope: Option<String>,
        cursor: Option<String>,
        no_sketch: bool,
    ) -> PyResult<PySearchResponse> {
        error::catch_panic(py, || {
            let request = SearchRequest {
                query,
                top_k,
                snippet_chars,
                uri,
                scope,
                cursor,
                #[cfg(feature = "temporal_track")]
                temporal: None,
                as_of_frame: None,
                as_of_ts: None,
                no_sketch,
                acl_context: None,
                acl_enforcement_mode: Default::default(),
            };
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let response = mv
                .search(request)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PySearchResponse::from(response))
        })
    }
}

/// Register search-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySearchHit>()?;
    m.add_class::<PySearchResponse>()?;
    Ok(())
}
