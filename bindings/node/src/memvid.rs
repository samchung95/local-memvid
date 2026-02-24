use std::sync::{Arc, Mutex, MutexGuard};

use napi_derive::napi;

use crate::error::from_memvid_error;

/// Acquire the inner `Memvid` guard, returning `[CLOSED]` error if already
/// closed or `[INTERNAL]` if the mutex is poisoned.
///
/// The returned `MutexGuard<Option<Memvid>>` is guaranteed to be `Some`.
/// Callers use `.as_ref().unwrap()` or `.as_mut().unwrap()` to access the
/// inner `Memvid`.
macro_rules! guard_memvid {
    ($self:expr) => {{
        let guard = $self.inner.lock().map_err(|_| {
            napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
        })?;
        if guard.is_none() {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                "[CLOSED] Memvid instance has been closed",
            ));
        }
        guard
    }};
}

pub(crate) use guard_memvid;

/// Helper to convert a `tokio::task::JoinError` into a NAPI error.
fn join_error(e: tokio::task::JoinError) -> napi::Error {
    napi::Error::new(
        napi::Status::GenericFailure,
        format!("[INTERNAL] {e}"),
    )
}

/// JavaScript wrapper around [`memvid_core::Memvid`].
///
/// Instances are created via static factory methods (`createSync`, `create`,
/// `openSync`, `open`, `openReadOnlySync`, `openReadOnly`) and closed
/// explicitly with [`close()`](JsMemvid::close).
///
/// After `close()` is called, all subsequent method calls will throw a
/// `MemvidError` with code `CLOSED`.
#[napi]
pub struct JsMemvid {
    pub(crate) inner: Arc<Mutex<Option<memvid_core::Memvid>>>,
}

#[napi]
impl JsMemvid {
    // -- Sync factory methods ------------------------------------------------

    /// Create a new `.mv2` file at the given path (synchronous).
    #[napi(factory, js_name = "createSync")]
    pub fn create_sync(path: String) -> napi::Result<Self> {
        let mv = memvid_core::Memvid::create(&path).map_err(from_memvid_error)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Some(mv))),
        })
    }

    /// Open an existing `.mv2` file for read-write access (synchronous).
    #[napi(factory, js_name = "openSync")]
    pub fn open_sync(path: String) -> napi::Result<Self> {
        let mv = memvid_core::Memvid::open(&path).map_err(from_memvid_error)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Some(mv))),
        })
    }

    /// Open an existing `.mv2` file for read-only access (synchronous).
    #[napi(factory, js_name = "openReadOnlySync")]
    pub fn open_read_only_sync(path: String) -> napi::Result<Self> {
        let mv = memvid_core::Memvid::open_read_only(&path).map_err(from_memvid_error)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Some(mv))),
        })
    }

    // -- Async factory methods -----------------------------------------------

    /// Create a new `.mv2` file at the given path (async).
    #[napi(factory, js_name = "create")]
    pub async fn create_async(path: String) -> napi::Result<JsMemvid> {
        let mv = tokio::task::spawn_blocking(move || memvid_core::Memvid::create(&path))
            .await
            .map_err(join_error)?
            .map_err(from_memvid_error)?;
        Ok(JsMemvid {
            inner: Arc::new(Mutex::new(Some(mv))),
        })
    }

    /// Open an existing `.mv2` file for read-write access (async).
    #[napi(factory, js_name = "open")]
    pub async fn open_async(path: String) -> napi::Result<JsMemvid> {
        let mv = tokio::task::spawn_blocking(move || memvid_core::Memvid::open(&path))
            .await
            .map_err(join_error)?
            .map_err(from_memvid_error)?;
        Ok(JsMemvid {
            inner: Arc::new(Mutex::new(Some(mv))),
        })
    }

    /// Open an existing `.mv2` file for read-only access (async).
    #[napi(factory, js_name = "openReadOnly")]
    pub async fn open_read_only_async(path: String) -> napi::Result<JsMemvid> {
        let mv =
            tokio::task::spawn_blocking(move || memvid_core::Memvid::open_read_only(&path))
                .await
                .map_err(join_error)?
                .map_err(from_memvid_error)?;
        Ok(JsMemvid {
            inner: Arc::new(Mutex::new(Some(mv))),
        })
    }

    // -- Lifecycle -----------------------------------------------------------

    /// Close the memvid file, releasing the file lock.
    ///
    /// After calling `close()`, all subsequent method calls on this instance
    /// will throw a `MemvidError` with code `CLOSED`.
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        let mut guard = self.inner.lock().map_err(|_| {
            napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
        })?;
        let _ = guard.take(); // drops Memvid, auto-commits if dirty
        Ok(())
    }

    // -- Getters -------------------------------------------------------------

    /// Whether this instance was opened in read-only mode.
    #[napi(getter, js_name = "isReadOnly")]
    pub fn is_read_only(&self) -> napi::Result<bool> {
        let guard = guard_memvid!(self);
        Ok(guard.as_ref().unwrap().is_read_only())
    }

    /// The file path of this memvid store.
    #[napi(getter)]
    pub fn path(&self) -> napi::Result<String> {
        let guard = guard_memvid!(self);
        Ok(guard
            .as_ref()
            .unwrap()
            .path()
            .to_string_lossy()
            .into_owned())
    }
}

/// Helper used by other modules: acquire a mutable guard on the inner Memvid.
///
/// This is not a macro—it's an ordinary function for use in contexts where the
/// macro's `return Err(...)` doesn't fit (e.g., closures sent to
/// `spawn_blocking`).
#[allow(dead_code)] // Will be used by future binding modules (e.g., spawn_blocking closures).
pub(crate) fn lock_inner(
    inner: &Arc<Mutex<Option<memvid_core::Memvid>>>,
) -> napi::Result<MutexGuard<'_, Option<memvid_core::Memvid>>> {
    let guard = inner.lock().map_err(|_| {
        napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
    })?;
    if guard.is_none() {
        return Err(napi::Error::new(
            napi::Status::GenericFailure,
            "[CLOSED] Memvid instance has been closed",
        ));
    }
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_sets_inner_to_none() {
        // We can't easily create a real Memvid in unit tests (needs filesystem),
        // so we test the close() logic directly on the mutex.
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        // close() on an already-None instance should succeed silently.
        js.close().unwrap();
        assert!(js.inner.lock().unwrap().is_none());
    }

    #[test]
    fn guard_rejects_closed_instance() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.is_read_only();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("[CLOSED]"));
    }

    #[test]
    fn lock_inner_rejects_closed() {
        let inner = Arc::new(Mutex::new(None::<memvid_core::Memvid>));
        let result = lock_inner(&inner);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
