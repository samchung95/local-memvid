use pyo3::prelude::*;

use memvid_core::types::SketchVariant;

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// Statistics about the sketch track.
#[pyclass(name = "SketchTrackStats", frozen)]
pub struct PySketchTrackStats {
    #[pyo3(get)]
    pub variant: String,
    #[pyo3(get)]
    pub entry_count: u64,
    #[pyo3(get)]
    pub size_bytes: u64,
    #[pyo3(get)]
    pub short_text_count: u64,
}

#[pymethods]
impl PySketchTrackStats {
    fn __repr__(&self) -> String {
        format!(
            "SketchTrackStats(variant={:?}, entry_count={}, size_bytes={}, short_text_count={})",
            self.variant, self.entry_count, self.size_bytes, self.short_text_count
        )
    }
}

fn variant_to_string(v: SketchVariant) -> String {
    match v {
        SketchVariant::Small => "small".to_string(),
        SketchVariant::Medium => "medium".to_string(),
        SketchVariant::Large => "large".to_string(),
    }
}

fn string_to_variant(s: &str) -> PyResult<SketchVariant> {
    match s {
        "small" => Ok(SketchVariant::Small),
        "medium" => Ok(SketchVariant::Medium),
        "large" => Ok(SketchVariant::Large),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "invalid sketch variant: {:?} (expected 'small', 'medium', or 'large')",
            s
        ))),
    }
}

// ---------------------------------------------------------------------------
// Sketch methods on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Build sketches for all frames that don't have one yet.
    ///
    /// Returns the number of new sketches generated.
    #[pyo3(signature = (variant="small"))]
    fn build_all_sketches(&self, py: Python<'_>, variant: &str) -> PyResult<u64> {
        let sketch_variant = string_to_variant(variant)?;
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let count = mv.build_all_sketches(sketch_variant);
            Ok(count as u64)
        })
    }

    /// Find candidate frame IDs matching a query using sketch filtering.
    ///
    /// Returns a list of frame IDs sorted by relevance score descending.
    #[pyo3(signature = (query, top_k))]
    fn find_sketch_candidates(
        &self,
        py: Python<'_>,
        query: String,
        top_k: usize,
    ) -> PyResult<Vec<u64>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let options = memvid_core::memvid::sketch::SketchSearchOptions {
                max_candidates: top_k,
                ..Default::default()
            };
            let candidates = mv.find_sketch_candidates(&query, Some(options));
            Ok(candidates.into_iter().map(|c| c.frame_id).collect())
        })
    }

    /// Check if the sketch track has any entries.
    fn has_sketches(&self, py: Python<'_>) -> PyResult<bool> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv.has_sketches())
        })
    }

    /// Get statistics about the sketch track.
    fn sketch_stats(&self, py: Python<'_>) -> PyResult<PySketchTrackStats> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let stats = mv.sketch_stats();
            Ok(PySketchTrackStats {
                variant: variant_to_string(stats.variant),
                entry_count: stats.entry_count,
                size_bytes: stats.size_bytes,
                short_text_count: stats.short_text_count,
            })
        })
    }
}

/// Register sketch-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySketchTrackStats>()?;
    Ok(())
}
