use pyo3::prelude::*;

use memvid_core::types::ask::{
    AskCitation as CoreAskCitation, AskContextFragment as CoreAskContextFragment,
    AskContextFragmentKind, AskMode, AskRequest, AskResponse as CoreAskResponse,
    AskRetriever, AskStats as CoreAskStats, VecEmbedder,
};
use memvid_core::types::search::SearchResponse as CoreSearchResponse;

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};
use crate::search::PySearchResponse;

// ---------------------------------------------------------------------------
// Dummy embedder (pass None::<&NoEmbedder> to ask())
// ---------------------------------------------------------------------------

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
// Python wrapper types
// ---------------------------------------------------------------------------

/// Performance statistics for an ask query.
#[pyclass(name = "AskStats", frozen)]
pub struct PyAskStats {
    #[pyo3(get)]
    pub retrieval_ms: u64,
    #[pyo3(get)]
    pub synthesis_ms: u64,
    #[pyo3(get)]
    pub latency_ms: u64,
}

#[pymethods]
impl PyAskStats {
    fn __repr__(&self) -> String {
        format!(
            "AskStats(retrieval_ms={}, synthesis_ms={}, latency_ms={})",
            self.retrieval_ms, self.synthesis_ms, self.latency_ms
        )
    }
}

impl From<CoreAskStats> for PyAskStats {
    fn from(s: CoreAskStats) -> Self {
        Self {
            retrieval_ms: s.retrieval_ms as u64,
            synthesis_ms: s.synthesis_ms as u64,
            latency_ms: s.latency_ms as u64,
        }
    }
}

/// A citation in an ask response.
#[pyclass(name = "AskCitation", frozen)]
pub struct PyAskCitation {
    #[pyo3(get)]
    pub index: usize,
    #[pyo3(get)]
    pub frame_id: u64,
    #[pyo3(get)]
    pub uri: String,
    #[pyo3(get)]
    pub chunk_range_start: Option<usize>,
    #[pyo3(get)]
    pub chunk_range_end: Option<usize>,
    #[pyo3(get)]
    pub score: Option<f32>,
}

#[pymethods]
impl PyAskCitation {
    fn __repr__(&self) -> String {
        format!(
            "AskCitation(index={}, frame_id={}, uri={:?})",
            self.index, self.frame_id, self.uri
        )
    }
}

impl From<CoreAskCitation> for PyAskCitation {
    fn from(c: CoreAskCitation) -> Self {
        let (start, end) = match c.chunk_range {
            Some((s, e)) => (Some(s), Some(e)),
            None => (None, None),
        };
        Self {
            index: c.index,
            frame_id: c.frame_id,
            uri: c.uri,
            chunk_range_start: start,
            chunk_range_end: end,
            score: c.score,
        }
    }
}

/// A context fragment used for answer synthesis.
#[pyclass(name = "AskContextFragment", frozen)]
pub struct PyAskContextFragment {
    #[pyo3(get)]
    pub rank: usize,
    #[pyo3(get)]
    pub frame_id: u64,
    #[pyo3(get)]
    pub uri: String,
    #[pyo3(get)]
    pub title: Option<String>,
    #[pyo3(get)]
    pub score: Option<f32>,
    #[pyo3(get)]
    pub match_count: usize,
    #[pyo3(get)]
    pub text: String,
    #[pyo3(get)]
    pub kind: Option<String>,
}

#[pymethods]
impl PyAskContextFragment {
    fn __repr__(&self) -> String {
        format!(
            "AskContextFragment(rank={}, frame_id={}, score={:?})",
            self.rank, self.frame_id, self.score
        )
    }
}

impl From<CoreAskContextFragment> for PyAskContextFragment {
    fn from(f: CoreAskContextFragment) -> Self {
        Self {
            rank: f.rank,
            frame_id: f.frame_id,
            uri: f.uri,
            title: f.title,
            score: f.score,
            match_count: f.matches,
            text: f.text,
            kind: f.kind.map(|k| match k {
                AskContextFragmentKind::Full => "full".to_string(),
                AskContextFragmentKind::Summary => "summary".to_string(),
            }),
        }
    }
}

/// Response from an ask (RAG) query.
#[pyclass(name = "AskResponse", frozen)]
pub struct PyAskResponse {
    #[pyo3(get)]
    pub question: String,
    #[pyo3(get)]
    pub mode: String,
    #[pyo3(get)]
    pub retriever: String,
    #[pyo3(get)]
    pub context_only: bool,
    #[pyo3(get)]
    pub answer: Option<String>,

    raw_retrieval: CoreSearchResponse,
    raw_citations: Vec<CoreAskCitation>,
    raw_context_fragments: Vec<CoreAskContextFragment>,
    raw_stats: CoreAskStats,
}

#[pymethods]
impl PyAskResponse {
    /// The raw search results from retrieval.
    #[getter]
    fn retrieval(&self) -> PySearchResponse {
        PySearchResponse::from(self.raw_retrieval.clone())
    }

    /// Citations in the answer.
    #[getter]
    fn citations(&self) -> Vec<PyAskCitation> {
        self.raw_citations
            .iter()
            .cloned()
            .map(PyAskCitation::from)
            .collect()
    }

    /// Context fragments sent to the synthesizer.
    #[getter]
    fn context_fragments(&self) -> Vec<PyAskContextFragment> {
        self.raw_context_fragments
            .iter()
            .cloned()
            .map(PyAskContextFragment::from)
            .collect()
    }

    /// Performance statistics.
    #[getter]
    fn stats(&self) -> PyAskStats {
        PyAskStats::from(self.raw_stats.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "AskResponse(question={:?}, mode={:?}, context_only={}, answer={})",
            self.question,
            self.mode,
            self.context_only,
            if self.answer.is_some() {
                "Some(...)"
            } else {
                "None"
            }
        )
    }
}

fn mode_to_string(mode: AskMode) -> String {
    match mode {
        AskMode::Lex => "lex".to_string(),
        AskMode::Sem => "sem".to_string(),
        AskMode::Hybrid => "hybrid".to_string(),
    }
}

fn string_to_mode(s: &str) -> PyResult<AskMode> {
    match s {
        "lex" => Ok(AskMode::Lex),
        "sem" => Ok(AskMode::Sem),
        "hybrid" => Ok(AskMode::Hybrid),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid ask mode: {:?} (expected 'lex', 'sem', or 'hybrid')",
            s
        ))),
    }
}

fn retriever_to_string(r: AskRetriever) -> String {
    match r {
        AskRetriever::Lex => "lex".to_string(),
        AskRetriever::Semantic => "semantic".to_string(),
        AskRetriever::Hybrid => "hybrid".to_string(),
        AskRetriever::LexFallback => "lex_fallback".to_string(),
        AskRetriever::TimelineFallback => "timeline_fallback".to_string(),
    }
}

impl From<CoreAskResponse> for PyAskResponse {
    fn from(r: CoreAskResponse) -> Self {
        Self {
            question: r.question,
            mode: mode_to_string(r.mode),
            retriever: retriever_to_string(r.retriever),
            context_only: r.context_only,
            answer: r.answer,
            raw_retrieval: r.retrieval,
            raw_citations: r.citations,
            raw_context_fragments: r.context_fragments,
            raw_stats: r.stats,
        }
    }
}

// ---------------------------------------------------------------------------
// ask() method on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Run a retrieval-augmented query for AI-powered Q&A.
    ///
    /// Returns an `AskResponse` with retrieval results, optional answer,
    /// citations, context fragments, and performance stats.
    #[pyo3(signature = (question, *, top_k=5, snippet_chars=200, uri=None, scope=None, context_only=false, mode="hybrid"))]
    fn ask(
        &self,
        py: Python<'_>,
        question: String,
        top_k: usize,
        snippet_chars: usize,
        uri: Option<String>,
        scope: Option<String>,
        context_only: bool,
        mode: &str,
    ) -> PyResult<PyAskResponse> {
        let ask_mode = string_to_mode(mode)?;
        error::catch_panic(py, || {
            let request = AskRequest {
                question,
                top_k,
                snippet_chars,
                uri,
                scope,
                cursor: None,
                start: None,
                end: None,
                #[cfg(feature = "temporal_track")]
                temporal: None,
                context_only,
                mode: ask_mode,
                as_of_frame: None,
                as_of_ts: None,
                adaptive: None,
                acl_context: None,
                acl_enforcement_mode: Default::default(),
            };
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let response = mv
                .ask(request, None::<&NoEmbedder>)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyAskResponse::from(response))
        })
    }
}

/// Register ask-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAskStats>()?;
    m.add_class::<PyAskCitation>()?;
    m.add_class::<PyAskContextFragment>()?;
    m.add_class::<PyAskResponse>()?;
    Ok(())
}
