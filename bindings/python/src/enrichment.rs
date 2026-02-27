use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

use memvid_core::enrichment_worker::{
    EnrichmentWorkerConfig, EnrichmentWorkerHandle, EnrichmentWorkerStats,
};

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

// ---------------------------------------------------------------------------
// PyEnrichmentWorkerStats
// ---------------------------------------------------------------------------

/// Statistics returned by `EnrichmentHandle.stats()`.
#[pyclass(name = "EnrichmentWorkerStats", frozen)]
pub struct PyEnrichmentWorkerStats {
    #[pyo3(get)]
    pub processed: u64,
    #[pyo3(get)]
    pub pending: usize,
    #[pyo3(get)]
    pub errors: u64,
    #[pyo3(get)]
    pub embeddings_generated: u64,
    #[pyo3(get)]
    pub re_extractions: u64,
    #[pyo3(get)]
    pub is_running: bool,
}

impl From<EnrichmentWorkerStats> for PyEnrichmentWorkerStats {
    fn from(s: EnrichmentWorkerStats) -> Self {
        Self {
            processed: s.frames_processed,
            pending: s.queue_depth,
            errors: s.errors,
            embeddings_generated: s.embeddings_generated,
            re_extractions: s.re_extractions,
            is_running: s.is_running,
        }
    }
}

// ---------------------------------------------------------------------------
// PyEnrichmentHandle
// ---------------------------------------------------------------------------

/// Handle for a background enrichment worker thread.
///
/// Use `stats()` to monitor progress, `stop()` to terminate gracefully,
/// and `is_running` to check if the worker is still active.
#[pyclass(name = "EnrichmentHandle")]
pub struct PyEnrichmentHandle {
    handle: EnrichmentWorkerHandle,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[pymethods]
impl PyEnrichmentHandle {
    /// Get current worker statistics.
    fn stats(&self) -> PyEnrichmentWorkerStats {
        self.handle.stats().into()
    }

    /// Signal the worker to stop and wait for it to finish.
    fn stop(&self) {
        self.handle.stop();
        // Try to join the thread (non-blocking if already finished)
        if let Ok(mut guard) = self.thread.lock() {
            if let Some(thread) = guard.take() {
                let _ = thread.join();
            }
        }
    }

    /// Whether the worker is still running.
    #[getter]
    fn is_running(&self) -> bool {
        self.handle.is_running()
    }
}

// ---------------------------------------------------------------------------
// PyMemvid enrichment methods
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Start a background enrichment worker.
    ///
    /// The worker processes frames from the enrichment queue asynchronously,
    /// re-extracting full text for skim extractions and updating indexes.
    ///
    /// Returns an `EnrichmentHandle` to monitor and control the worker.
    #[pyo3(signature = (*, embedding_batch_size=32, checkpoint_interval=100, task_delay_ms=50, max_task_time_ms=5000))]
    fn start_enrichment_worker(
        &self,
        py: Python<'_>,
        embedding_batch_size: usize,
        checkpoint_interval: usize,
        task_delay_ms: u64,
        max_task_time_ms: u64,
    ) -> PyResult<PyEnrichmentHandle> {
        error::catch_panic(py, || {
            // Verify the handle is open before spawning
            {
                let lock = guard_memvid!(self, py);
                drop(lock);
            }

            let config = EnrichmentWorkerConfig {
                embedding_batch_size,
                checkpoint_interval,
                task_delay_ms,
                max_task_time_ms,
            };

            let handle = EnrichmentWorkerHandle::new();
            let worker_handle = handle.clone_handle();
            let inner = Arc::clone(&self.inner);
            let config_clone = config.clone();

            let thread = std::thread::spawn(move || {
                memvid_core::enrichment_worker::run_worker_loop(
                    &worker_handle,
                    &config_clone,
                    // get_next_task
                    || {
                        let lock = inner.lock().ok()?;
                        let mv = lock.as_ref()?;
                        mv.next_enrichment_task()
                    },
                    // process_task
                    |task| {
                        let mut lock = match inner.lock() {
                            Ok(lock) => lock,
                            Err(_) => {
                                return memvid_core::enrichment_worker::TaskResult {
                                    frame_id: task.frame_id,
                                    re_extracted: false,
                                    embeddings_generated: 0,
                                    elapsed_ms: 0,
                                    error: Some("Failed to acquire lock".to_string()),
                                };
                            }
                        };
                        match lock.as_mut() {
                            Some(mv) => mv.process_enrichment_task(task),
                            None => memvid_core::enrichment_worker::TaskResult {
                                frame_id: task.frame_id,
                                re_extracted: false,
                                embeddings_generated: 0,
                                elapsed_ms: 0,
                                error: Some("Memvid handle is closed".to_string()),
                            },
                        }
                    },
                    // mark_complete
                    |frame_id| {
                        if let Ok(mut lock) = inner.lock() {
                            if let Some(mv) = lock.as_mut() {
                                mv.complete_enrichment_task(frame_id);
                            }
                        }
                    },
                    // checkpoint
                    || {
                        if let Ok(mut lock) = inner.lock() {
                            if let Some(mv) = lock.as_mut() {
                                let _ = mv.commit();
                            }
                        }
                    },
                );
            });

            Ok(PyEnrichmentHandle {
                handle,
                thread: Mutex::new(Some(thread)),
            })
        })
    }

    /// Get frame IDs that have not been enriched by the given engine.
    fn get_unenriched_frames(
        &self,
        py: Python<'_>,
        engine_kind: &str,
        engine_version: &str,
    ) -> PyResult<Vec<u64>> {
        let lock = guard_memvid!(self, py);
        let mv = lock.as_ref().unwrap();
        Ok(mv.get_unenriched_frames(engine_kind, engine_version))
    }

    /// Record that a frame has been enriched by a specific engine.
    #[pyo3(signature = (frame_id, engine_kind, engine_version, card_ids=None))]
    fn record_enrichment(
        &self,
        py: Python<'_>,
        frame_id: u64,
        engine_kind: &str,
        engine_version: &str,
        card_ids: Option<Vec<u64>>,
    ) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.record_enrichment(
                frame_id,
                engine_kind,
                engine_version,
                card_ids.unwrap_or_default(),
            )
            .map_err(|e| error::from_memvid_error(py, e))
        })
    }

    /// Check if a frame has been enriched by a specific engine.
    fn is_frame_enriched(
        &self,
        py: Python<'_>,
        frame_id: u64,
        engine_kind: &str,
        engine_version: &str,
    ) -> PyResult<bool> {
        let lock = guard_memvid!(self, py);
        let mv = lock.as_ref().unwrap();
        Ok(mv.is_frame_enriched(frame_id, engine_kind, engine_version))
    }
}

/// Register enrichment types on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEnrichmentWorkerStats>()?;
    m.add_class::<PyEnrichmentHandle>()?;
    Ok(())
}
