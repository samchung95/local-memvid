use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use napi_derive::napi;

use memvid_core::enrichment_worker::{
    EnrichmentWorkerConfig, EnrichmentWorkerHandle, EnrichmentWorkerStats, TaskResult,
    run_worker_loop,
};

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// JS types
// ---------------------------------------------------------------------------

/// Configuration for the background enrichment worker.
#[napi(object)]
pub struct JsEnrichmentWorkerConfig {
    /// Batch size for embedding generation (default: 32).
    pub embedding_batch_size: Option<u32>,
    /// Checkpoint interval — persist progress every N embeddings (default: 100).
    pub checkpoint_interval: Option<u32>,
    /// Delay in ms between processing tasks (default: 50).
    pub task_delay_ms: Option<f64>,
    /// Maximum time in ms to spend on a single task (default: 5000).
    pub max_task_time_ms: Option<f64>,
}

/// Worker statistics returned by `JsEnrichmentHandle.stats()`.
#[napi(object)]
pub struct JsEnrichmentWorkerStats {
    /// Total frames processed.
    pub processed: f64,
    /// Pending frames in queue.
    pub pending: f64,
    /// Total errors encountered.
    pub errors: f64,
    /// Total embeddings generated.
    pub embeddings_generated: f64,
    /// Total re-extractions performed.
    pub re_extractions: f64,
    /// Whether the worker is still running.
    pub is_running: bool,
}

fn from_worker_stats(s: EnrichmentWorkerStats) -> JsEnrichmentWorkerStats {
    JsEnrichmentWorkerStats {
        processed: s.frames_processed as f64,
        pending: s.queue_depth as f64,
        errors: s.errors as f64,
        embeddings_generated: s.embeddings_generated as f64,
        re_extractions: s.re_extractions as f64,
        is_running: s.is_running,
    }
}

fn to_worker_config(cfg: JsEnrichmentWorkerConfig) -> EnrichmentWorkerConfig {
    let mut c = EnrichmentWorkerConfig::default();
    if let Some(v) = cfg.embedding_batch_size {
        c.embedding_batch_size = v as usize;
    }
    if let Some(v) = cfg.checkpoint_interval {
        c.checkpoint_interval = v as usize;
    }
    if let Some(v) = cfg.task_delay_ms {
        c.task_delay_ms = v as u64;
    }
    if let Some(v) = cfg.max_task_time_ms {
        c.max_task_time_ms = v as u64;
    }
    c
}

// ---------------------------------------------------------------------------
// JsEnrichmentHandle — wraps the background worker handle
// ---------------------------------------------------------------------------

/// Handle for controlling and monitoring a background enrichment worker.
///
/// Created by `JsMemvid.startEnrichmentWorker()`.
#[napi]
pub struct JsEnrichmentHandle {
    worker_handle: EnrichmentWorkerHandle,
    thread: Mutex<Option<JoinHandle<()>>>,
}

#[napi]
impl JsEnrichmentHandle {
    /// Get current worker statistics.
    #[napi]
    pub fn stats(&self) -> JsEnrichmentWorkerStats {
        from_worker_stats(self.worker_handle.stats())
    }

    /// Signal the worker to stop gracefully and wait for it to finish.
    #[napi]
    pub fn stop(&self) -> napi::Result<()> {
        self.worker_handle.stop();
        let mut guard = self.thread.lock().map_err(|_| {
            napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
        })?;
        if let Some(thread) = guard.take() {
            let _ = thread.join();
        }
        Ok(())
    }

    /// Check if the worker is still running.
    #[napi(getter, js_name = "isRunning")]
    pub fn is_running(&self) -> bool {
        self.worker_handle.is_running()
    }
}

// ---------------------------------------------------------------------------
// Start worker — variant of start_enrichment_worker for Option<Memvid>
// ---------------------------------------------------------------------------

/// Start a background enrichment worker for `Arc<Mutex<Option<Memvid>>>`.
///
/// This mirrors `memvid_core::start_enrichment_worker` but works with the
/// `Option` wrapper used by `JsMemvid`. If the inner `Memvid` is `None`
/// (instance closed), the worker gracefully stops.
fn start_worker_for_js(
    inner: Arc<Mutex<Option<memvid_core::Memvid>>>,
    config: Option<EnrichmentWorkerConfig>,
) -> JsEnrichmentHandle {
    let config = config.unwrap_or_default();
    let handle = EnrichmentWorkerHandle::new();
    let worker_handle = handle.clone_handle();

    let inner_clone = Arc::clone(&inner);
    let config_clone = config.clone();

    let thread = std::thread::spawn(move || {
        run_worker_loop(
            &worker_handle,
            &config_clone,
            // get_next_task: return None (stopping) if instance is closed
            || {
                let guard = inner_clone.lock().ok()?;
                let mv = guard.as_ref()?;
                mv.next_enrichment_task()
            },
            // process_task
            |task| {
                let mut guard = match inner_clone.lock() {
                    Ok(g) => g,
                    Err(_) => {
                        return TaskResult {
                            frame_id: task.frame_id,
                            re_extracted: false,
                            embeddings_generated: 0,
                            elapsed_ms: 0,
                            error: Some("Failed to acquire lock".to_string()),
                        };
                    }
                };
                match guard.as_mut() {
                    Some(mv) => mv.process_enrichment_task(task),
                    None => TaskResult {
                        frame_id: task.frame_id,
                        re_extracted: false,
                        embeddings_generated: 0,
                        elapsed_ms: 0,
                        error: Some("Instance closed".to_string()),
                    },
                }
            },
            // mark_complete
            |frame_id| {
                if let Ok(mut guard) = inner_clone.lock() {
                    if let Some(mv) = guard.as_mut() {
                        mv.complete_enrichment_task(frame_id);
                    }
                }
            },
            // checkpoint
            || {
                if let Ok(mut guard) = inner_clone.lock() {
                    if let Some(mv) = guard.as_mut() {
                        let _ = mv.commit();
                    }
                }
            },
        );
    });

    JsEnrichmentHandle {
        worker_handle: handle,
        thread: Mutex::new(Some(thread)),
    }
}

// ---------------------------------------------------------------------------
// Methods on JsMemvid
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Start a background enrichment worker that processes frames from the
    /// enrichment queue (re-extraction + index updates).
    ///
    /// Returns a `JsEnrichmentHandle` to control and monitor the worker.
    #[napi(js_name = "startEnrichmentWorker")]
    pub fn start_enrichment_worker(
        &self,
        config: Option<JsEnrichmentWorkerConfig>,
    ) -> napi::Result<JsEnrichmentHandle> {
        // Verify the instance is open before starting the worker.
        let _guard = guard_memvid!(self);
        drop(_guard);

        let cfg = config.map(to_worker_config);
        Ok(start_worker_for_js(Arc::clone(&self.inner), cfg))
    }

    // -- Engine-based enrichment tracking ------------------------------------

    /// Get frame IDs not yet enriched by the given engine kind + version (sync).
    #[napi(js_name = "getUnenrichedFramesSync")]
    pub fn get_unenriched_frames_sync(
        &self,
        engine_kind: String,
        engine_version: String,
    ) -> napi::Result<Vec<i64>> {
        let guard = guard_memvid!(self);
        let ids = guard
            .as_ref()
            .unwrap()
            .get_unenriched_frames(&engine_kind, &engine_version);
        Ok(ids.into_iter().map(|id| id as i64).collect())
    }

    /// Get frame IDs not yet enriched by the given engine kind + version (async).
    #[napi(js_name = "getUnenrichedFrames")]
    pub async fn get_unenriched_frames(
        &self,
        engine_kind: String,
        engine_version: String,
    ) -> napi::Result<Vec<i64>> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let ids = guard
                .as_ref()
                .unwrap()
                .get_unenriched_frames(&engine_kind, &engine_version);
            Ok(ids.into_iter().map(|id| id as i64).collect())
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Record that a frame has been enriched by a specific engine (sync).
    #[napi(js_name = "recordEnrichmentSync")]
    pub fn record_enrichment_sync(
        &self,
        frame_id: i64,
        engine_kind: String,
        engine_version: String,
    ) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        guard
            .as_mut()
            .unwrap()
            .record_enrichment(frame_id as u64, &engine_kind, &engine_version, vec![])
            .map_err(from_memvid_error)
    }

    /// Record that a frame has been enriched by a specific engine (async).
    #[napi(js_name = "recordEnrichment")]
    pub async fn record_enrichment(
        &self,
        frame_id: i64,
        engine_kind: String,
        engine_version: String,
    ) -> napi::Result<()> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            guard
                .as_mut()
                .unwrap()
                .record_enrichment(frame_id as u64, &engine_kind, &engine_version, vec![])
                .map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    /// Check if a frame has been enriched by a specific engine (sync).
    #[napi(js_name = "isFrameEnrichedSync")]
    pub fn is_frame_enriched_sync(
        &self,
        frame_id: i64,
        engine_kind: String,
        engine_version: String,
    ) -> napi::Result<bool> {
        let guard = guard_memvid!(self);
        Ok(guard
            .as_ref()
            .unwrap()
            .is_frame_enriched(frame_id as u64, &engine_kind, &engine_version))
    }

    /// Check if a frame has been enriched by a specific engine (async).
    #[napi(js_name = "isFrameEnriched")]
    pub async fn is_frame_enriched(
        &self,
        frame_id: i64,
        engine_kind: String,
        engine_version: String,
    ) -> napi::Result<bool> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            Ok(guard
                .as_ref()
                .unwrap()
                .is_frame_enriched(frame_id as u64, &engine_kind, &engine_version))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_worker_config_defaults() {
        let js = JsEnrichmentWorkerConfig {
            embedding_batch_size: None,
            checkpoint_interval: None,
            task_delay_ms: None,
            max_task_time_ms: None,
        };
        let cfg = to_worker_config(js);
        assert_eq!(cfg.embedding_batch_size, 32);
        assert_eq!(cfg.checkpoint_interval, 100);
        assert_eq!(cfg.task_delay_ms, 50);
        assert_eq!(cfg.max_task_time_ms, 5000);
    }

    #[test]
    fn to_worker_config_overrides() {
        let js = JsEnrichmentWorkerConfig {
            embedding_batch_size: Some(64),
            checkpoint_interval: Some(200),
            task_delay_ms: Some(25.0),
            max_task_time_ms: Some(10000.0),
        };
        let cfg = to_worker_config(js);
        assert_eq!(cfg.embedding_batch_size, 64);
        assert_eq!(cfg.checkpoint_interval, 200);
        assert_eq!(cfg.task_delay_ms, 25);
        assert_eq!(cfg.max_task_time_ms, 10000);
    }

    #[test]
    fn from_worker_stats_converts() {
        let stats = EnrichmentWorkerStats {
            frames_processed: 10,
            embeddings_generated: 50,
            re_extractions: 3,
            errors: 1,
            queue_depth: 5,
            is_running: true,
        };
        let js = from_worker_stats(stats);
        assert_eq!(js.processed, 10.0);
        assert_eq!(js.embeddings_generated, 50.0);
        assert_eq!(js.re_extractions, 3.0);
        assert_eq!(js.errors, 1.0);
        assert_eq!(js.pending, 5.0);
        assert!(js.is_running);
    }

    #[test]
    fn handle_stats_default_when_not_started() {
        // A freshly-created handle (worker not yet started) returns zeroed stats.
        let handle = EnrichmentWorkerHandle::new();
        let stats = handle.stats();
        assert_eq!(stats.frames_processed, 0);
        assert!(!stats.is_running);
    }

    #[test]
    fn handle_stop_is_idempotent() {
        let handle = EnrichmentWorkerHandle::new();
        handle.stop();
        handle.stop(); // double stop should be fine
        assert!(handle.should_stop());
    }

    #[test]
    fn closed_instance_rejects_get_unenriched_frames() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.get_unenriched_frames_sync("test".into(), "1.0".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }

    #[test]
    fn closed_instance_rejects_record_enrichment() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.record_enrichment_sync(0, "test".into(), "1.0".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }

    #[test]
    fn closed_instance_rejects_is_frame_enriched() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.is_frame_enriched_sync(0, "test".into(), "1.0".into());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }

    #[test]
    fn closed_instance_rejects_start_enrichment_worker() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.start_enrichment_worker(None);
        match result {
            Err(e) => assert!(e.to_string().contains("[CLOSED]")),
            Ok(_) => panic!("expected Err"),
        }
    }
}
