use std::io::Read;
use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

/// Default chunk size for iteration (64 KiB).
const DEFAULT_CHUNK_SIZE: usize = 65_536;

/// Streaming reader over a frame's canonical payload bytes.
///
/// Supports `read(size)`, context manager (`with`), and iteration.
#[pyclass(name = "BlobReader")]
pub struct PyBlobReader {
    inner: Mutex<Option<memvid_core::BlobReader>>,
    total_len: u64,
}

#[pymethods]
impl PyBlobReader {
    /// Total byte count of the blob.
    #[getter]
    fn length(&self) -> u64 {
        self.total_len
    }

    /// Read up to `size` bytes. Returns `b""` at EOF.
    /// If `size` is negative (default), reads the entire remaining content.
    #[pyo3(signature = (size = -1))]
    fn read<'py>(&self, py: Python<'py>, size: i64) -> PyResult<Bound<'py, PyBytes>> {
        let mut lock = self
            .inner
            .lock()
            .map_err(|_| error::closed_error(py, "BlobReader lock poisoned"))?;
        let reader = lock.as_mut().ok_or_else(|| {
            error::closed_error(py, "BlobReader is closed")
        })?;

        if size < 0 {
            // Read all remaining bytes.
            let mut buf = Vec::new();
            reader
                .read_to_end(&mut buf)
                .map_err(|e| error::from_io_error(py, e))?;
            Ok(PyBytes::new(py, &buf))
        } else {
            let mut buf = vec![0u8; size as usize];
            let n = reader
                .read(&mut buf)
                .map_err(|e| error::from_io_error(py, e))?;
            buf.truncate(n);
            Ok(PyBytes::new(py, &buf))
        }
    }

    /// Release underlying resources. Further reads will raise `MemvidError`.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let mut lock = self
            .inner
            .lock()
            .map_err(|_| error::closed_error(py, "BlobReader lock poisoned"))?;
        *lock = None;
        Ok(())
    }

    // --- context manager ----------------------------------------------------

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: &Bound<'_, PyAny>,
        _exc_val: &Bound<'_, PyAny>,
        _exc_tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.close(py)?;
        Ok(false)
    }

    // --- iteration ----------------------------------------------------------

    fn __iter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Yield chunks of up to 64 KiB. Returns `StopIteration` at EOF.
    fn __next__<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyBytes>>> {
        let mut lock = self
            .inner
            .lock()
            .map_err(|_| error::closed_error(py, "BlobReader lock poisoned"))?;
        let reader = lock.as_mut().ok_or_else(|| {
            error::closed_error(py, "BlobReader is closed")
        })?;

        let mut buf = vec![0u8; DEFAULT_CHUNK_SIZE];
        let n = reader
            .read(&mut buf)
            .map_err(|e| error::from_io_error(py, e))?;
        if n == 0 {
            Ok(None) // StopIteration
        } else {
            buf.truncate(n);
            Ok(Some(PyBytes::new(py, &buf)))
        }
    }
}

// ---------------------------------------------------------------------------
// blob_reader / blob_reader_by_uri on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Open a streaming reader for a frame's canonical payload.
    fn blob_reader(&self, py: Python<'_>, frame_id: u64) -> PyResult<PyBlobReader> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let reader = mv
                .blob_reader(frame_id)
                .map_err(|e| error::from_memvid_error(py, e))?;
            let total_len = reader.len();
            Ok(PyBlobReader {
                inner: Mutex::new(Some(reader)),
                total_len,
            })
        })
    }

    /// Open a streaming reader for a frame identified by URI.
    fn blob_reader_by_uri(&self, py: Python<'_>, uri: &str) -> PyResult<PyBlobReader> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            let reader = mv
                .blob_reader_by_uri(uri)
                .map_err(|e| error::from_memvid_error(py, e))?;
            let total_len = reader.len();
            Ok(PyBlobReader {
                inner: Mutex::new(Some(reader)),
                total_len,
            })
        })
    }
}

/// Register blob-reader classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBlobReader>()?;
    Ok(())
}
