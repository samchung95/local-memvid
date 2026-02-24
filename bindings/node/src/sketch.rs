use napi_derive::napi;

use memvid_core::SketchTrackStats;
use memvid_core::SketchVariant;

use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// JsSketchTrackStats
// ---------------------------------------------------------------------------

/// Statistics about the sketch track.
#[napi(object)]
pub struct JsSketchTrackStats {
    /// Sketch variant: "small", "medium", or "large".
    pub variant: String,
    /// Number of sketch entries.
    pub entry_count: f64,
    /// Size in bytes.
    pub size_bytes: f64,
    /// Number of entries marked as short text.
    pub short_text_count: f64,
}

/// Convert `SketchVariant` to a JS-friendly string.
fn variant_to_string(v: &SketchVariant) -> String {
    match v {
        SketchVariant::Small => "small".to_string(),
        SketchVariant::Medium => "medium".to_string(),
        SketchVariant::Large => "large".to_string(),
    }
}

/// Parse a variant string into `SketchVariant`.
fn parse_variant(s: &str) -> SketchVariant {
    match s.to_lowercase().as_str() {
        "small" => SketchVariant::Small,
        "medium" => SketchVariant::Medium,
        "large" => SketchVariant::Large,
        _ => SketchVariant::default(),
    }
}

/// Convert `SketchTrackStats` to the JS representation.
fn from_sketch_track_stats(stats: SketchTrackStats) -> JsSketchTrackStats {
    JsSketchTrackStats {
        variant: variant_to_string(&stats.variant),
        entry_count: stats.entry_count as f64,
        size_bytes: stats.size_bytes as f64,
        short_text_count: stats.short_text_count as f64,
    }
}

// ---------------------------------------------------------------------------
// JsMemvid sketch methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Build sketches for all frames that don't have one yet (synchronous).
    ///
    /// Returns the number of new sketches built.
    #[napi(js_name = "buildAllSketchesSync")]
    pub fn build_all_sketches_sync(&self, variant: String) -> napi::Result<u32> {
        let v = parse_variant(&variant);
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        Ok(mv.build_all_sketches(v) as u32)
    }

    /// Build sketches for all frames that don't have one yet (async).
    ///
    /// Returns a Promise resolving to the number of new sketches built.
    #[napi(js_name = "buildAllSketches")]
    pub async fn build_all_sketches_async(&self, variant: String) -> napi::Result<u32> {
        let v = parse_variant(&variant);
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            let mv = guard.as_mut().unwrap();
            Ok(mv.build_all_sketches(v) as u32)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Find candidate frames matching a query using sketch filtering (synchronous).
    ///
    /// Returns an array of frame IDs (bigint) sorted by sketch score descending.
    #[napi(js_name = "findSketchCandidatesSync")]
    pub fn find_sketch_candidates_sync(
        &self,
        query: String,
        top_k: u32,
    ) -> napi::Result<Vec<i64>> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        let options = Some(memvid_core::SketchSearchOptions {
            max_candidates: top_k as usize,
            ..Default::default()
        });
        let candidates = mv.find_sketch_candidates(&query, options);
        Ok(candidates.iter().map(|c| c.frame_id as i64).collect())
    }

    /// Find candidate frames matching a query using sketch filtering (async).
    ///
    /// Returns a Promise resolving to an array of frame IDs (bigint).
    #[napi(js_name = "findSketchCandidates")]
    pub async fn find_sketch_candidates_async(
        &self,
        query: String,
        top_k: u32,
    ) -> napi::Result<Vec<i64>> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let mv = guard.as_ref().unwrap();
            let options = Some(memvid_core::SketchSearchOptions {
                max_candidates: top_k as usize,
                ..Default::default()
            });
            let candidates = mv.find_sketch_candidates(&query, options);
            Ok(candidates.iter().map(|c| c.frame_id as i64).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Check if the sketch track has any entries.
    #[napi(js_name = "hasSketches")]
    pub fn has_sketches(&self) -> napi::Result<bool> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(mv.has_sketches())
    }

    /// Get statistics about the sketch track.
    #[napi(js_name = "sketchStats")]
    pub fn sketch_stats(&self) -> napi::Result<JsSketchTrackStats> {
        let guard = guard_memvid!(self);
        let mv = guard.as_ref().unwrap();
        Ok(from_sketch_track_stats(mv.sketch_stats()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_string_roundtrip() {
        assert_eq!(variant_to_string(&SketchVariant::Small), "small");
        assert_eq!(variant_to_string(&SketchVariant::Medium), "medium");
        assert_eq!(variant_to_string(&SketchVariant::Large), "large");
    }

    #[test]
    fn parse_variant_valid() {
        assert_eq!(parse_variant("small"), SketchVariant::Small);
        assert_eq!(parse_variant("SMALL"), SketchVariant::Small);
        assert_eq!(parse_variant("medium"), SketchVariant::Medium);
        assert_eq!(parse_variant("Medium"), SketchVariant::Medium);
        assert_eq!(parse_variant("large"), SketchVariant::Large);
        assert_eq!(parse_variant("LARGE"), SketchVariant::Large);
    }

    #[test]
    fn parse_variant_unknown_defaults_to_small() {
        assert_eq!(parse_variant("unknown"), SketchVariant::Small);
        assert_eq!(parse_variant(""), SketchVariant::Small);
    }

    #[test]
    fn sketch_track_stats_conversion() {
        let stats = SketchTrackStats {
            variant: SketchVariant::Medium,
            entry_count: 42,
            size_bytes: 2688,
            short_text_count: 5,
        };
        let js = from_sketch_track_stats(stats);
        assert_eq!(js.variant, "medium");
        assert!((js.entry_count - 42.0).abs() < 0.001);
        assert!((js.size_bytes - 2688.0).abs() < 0.001);
        assert!((js.short_text_count - 5.0).abs() < 0.001);
    }

    #[test]
    fn build_all_sketches_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.build_all_sketches_sync("small".to_string());
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn find_sketch_candidates_sync_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.find_sketch_candidates_sync("test".to_string(), 10);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn has_sketches_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.has_sketches();
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn sketch_stats_rejects_closed() {
        let js = JsMemvid {
            inner: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };
        let result = js.sketch_stats();
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
