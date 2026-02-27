use std::sync::Mutex;

use pyo3::prelude::*;

use crate::error;

/// Guard that acquires the inner Mutex lock and returns a mutable reference
/// to the `Memvid` handle, or raises `MemvidError` with code `CLOSED` if the
/// handle has been closed.
macro_rules! guard_memvid {
    ($self:expr, $py:expr) => {{
        let lock = $self
            .inner
            .lock()
            .map_err(|_| error::closed_error($py, "lock poisoned"))?;
        if lock.is_none() {
            return Err(error::closed_error($py, "Memvid handle is closed"));
        }
        lock
    }};
    (mut $self:expr, $py:expr) => {{
        let lock = $self
            .inner
            .lock()
            .map_err(|_| error::closed_error($py, "lock poisoned"))?;
        if lock.is_none() {
            return Err(error::closed_error($py, "Memvid handle is closed"));
        }
        lock
    }};
}

pub(crate) use guard_memvid;

/// Python wrapper around `memvid_core::Memvid`.
///
/// The inner handle is stored behind `Mutex<Option<…>>` so that `close()`
/// can set it to `None` and all subsequent calls raise `MemvidError(CLOSED)`.
#[pyclass(name = "Memvid")]
pub struct PyMemvid {
    pub(crate) inner: Mutex<Option<memvid_core::Memvid>>,
}

#[pymethods]
impl PyMemvid {
    // --- factory (static) methods -------------------------------------------

    /// Create a new `.mv2` file at `path`.
    #[staticmethod]
    fn create(py: Python<'_>, path: &str) -> PyResult<Self> {
        error::catch_panic(py, || {
            let mv =
                memvid_core::Memvid::create(path).map_err(|e| error::from_memvid_error(py, e))?;
            Ok(Self {
                inner: Mutex::new(Some(mv)),
            })
        })
    }

    /// Open an existing `.mv2` file with exclusive (read-write) access.
    #[staticmethod]
    fn open(py: Python<'_>, path: &str) -> PyResult<Self> {
        error::catch_panic(py, || {
            let mv =
                memvid_core::Memvid::open(path).map_err(|e| error::from_memvid_error(py, e))?;
            Ok(Self {
                inner: Mutex::new(Some(mv)),
            })
        })
    }

    /// Open an existing `.mv2` file in read-only mode.
    #[staticmethod]
    fn open_read_only(py: Python<'_>, path: &str) -> PyResult<Self> {
        error::catch_panic(py, || {
            let mv = memvid_core::Memvid::open_read_only(path)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(Self {
                inner: Mutex::new(Some(mv)),
            })
        })
    }

    // --- instance methods ---------------------------------------------------

    /// Close the handle, releasing the file lock. Further calls will raise
    /// `MemvidError` with code `CLOSED`.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = self
                .inner
                .lock()
                .map_err(|_| error::closed_error(py, "lock poisoned"))?;
            *lock = None;
            Ok(())
        })
    }

    // --- properties ---------------------------------------------------------

    /// Whether the file was opened in read-only mode.
    #[getter]
    fn is_read_only(&self, py: Python<'_>) -> PyResult<bool> {
        let lock = guard_memvid!(self, py);
        Ok(lock.as_ref().unwrap().is_read_only())
    }

    /// The filesystem path of the open `.mv2` file.
    #[getter]
    fn path(&self, py: Python<'_>) -> PyResult<String> {
        let lock = guard_memvid!(self, py);
        Ok(lock.as_ref().unwrap().path().to_string_lossy().into_owned())
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
}

/// Register the `Memvid` class on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMemvid>()?;
    Ok(())
}
