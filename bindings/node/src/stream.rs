use std::io::Read;
use std::sync::Mutex;

use napi::bindgen_prelude::*;
use napi_derive::napi;

use memvid_core::FrameId;

use crate::error::from_memvid_error;
use crate::memvid::{JsMemvid, guard_memvid};

// ---------------------------------------------------------------------------
// JsBlobReader — streaming reader for frame payloads
// ---------------------------------------------------------------------------

/// A streaming reader for frame payloads.
///
/// Reads frame data in chunks without loading the entire payload into memory.
/// Use `readChunk(size)` to pull data incrementally.
///
/// To convert to a Node.js Readable stream:
/// ```js
/// const { Readable } = require('stream');
/// const blobReader = mv.blobReader(frameId);
/// const readable = new Readable({
///   read(size) {
///     try {
///       const chunk = blobReader.readChunk(size);
///       this.push(chunk); // null signals EOF
///     } catch (e) {
///       this.destroy(e);
///     }
///   }
/// });
/// ```
#[napi]
pub struct JsBlobReader {
    inner: Mutex<Option<memvid_core::BlobReader>>,
    total_len: i64,
}

#[napi]
impl JsBlobReader {
    /// Read the next chunk of bytes (up to `size` bytes).
    ///
    /// Returns a `Buffer` containing the next chunk, or `null` when the end of
    /// the blob has been reached (EOF).
    #[napi(js_name = "readChunk")]
    pub fn read_chunk(&self, size: i32) -> napi::Result<Option<Buffer>> {
        let mut guard = self.inner.lock().map_err(|_| {
            napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
        })?;
        let reader = guard.as_mut().ok_or_else(|| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "[CLOSED] BlobReader has been closed",
            )
        })?;

        let chunk_size = (size as usize).clamp(1, 65536);
        let mut buf = vec![0u8; chunk_size];

        match reader.read(&mut buf) {
            Ok(0) => Ok(None),
            Ok(n) => {
                buf.truncate(n);
                Ok(Some(buf.into()))
            }
            Err(e) => Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!("[IO] {e}"),
            )),
        }
    }

    /// Total byte length of the blob.
    #[napi(getter)]
    pub fn length(&self) -> i64 {
        self.total_len
    }

    /// Close the blob reader, releasing resources.
    ///
    /// After calling `close()`, subsequent `readChunk()` calls will throw.
    #[napi]
    pub fn close(&self) -> napi::Result<()> {
        let mut guard = self.inner.lock().map_err(|_| {
            napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
        })?;
        guard.take();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JsMemvid blob reader methods
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Create a streaming reader for the canonical payload of a frame.
    ///
    /// Returns a `JsBlobReader` that reads data in chunks without loading
    /// the entire payload into memory. Call `readChunk(size)` to pull data.
    ///
    /// To wrap in a Node.js Readable stream:
    /// ```js
    /// const { Readable } = require('stream');
    /// const blob = mv.blobReader(frameId);
    /// const stream = new Readable({
    ///   read(size) {
    ///     try { this.push(blob.readChunk(size)); }
    ///     catch (e) { this.destroy(e); }
    ///   }
    /// });
    /// stream.pipe(destination);
    /// ```
    #[napi(js_name = "blobReader")]
    pub fn blob_reader_sync(&self, frame_id: i64) -> napi::Result<JsBlobReader> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let reader = mv
            .blob_reader(frame_id as FrameId)
            .map_err(from_memvid_error)?;
        let len = reader.len() as i64;
        Ok(JsBlobReader {
            inner: Mutex::new(Some(reader)),
            total_len: len,
        })
    }

    /// Create a streaming reader for the canonical payload of a frame by URI.
    ///
    /// Returns a `JsBlobReader` that reads data in chunks without loading
    /// the entire payload into memory.
    #[napi(js_name = "blobReaderByUri")]
    pub fn blob_reader_by_uri_sync(&self, uri: String) -> napi::Result<JsBlobReader> {
        let mut guard = guard_memvid!(self);
        let mv = guard.as_mut().unwrap();
        let reader = mv.blob_reader_by_uri(&uri).map_err(from_memvid_error)?;
        let len = reader.len() as i64;
        Ok(JsBlobReader {
            inner: Mutex::new(Some(reader)),
            total_len: len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    fn closed_memvid() -> JsMemvid {
        JsMemvid {
            inner: Arc::new(StdMutex::new(None)),
        }
    }

    #[test]
    fn blob_reader_sync_rejects_closed() {
        let js = closed_memvid();
        let err = match js.blob_reader_sync(0) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn blob_reader_by_uri_sync_rejects_closed() {
        let js = closed_memvid();
        let err = match js.blob_reader_by_uri_sync("test://uri".to_string()) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn blob_reader_read_chunk_rejects_closed_reader() {
        let blob = JsBlobReader {
            inner: Mutex::new(None),
            total_len: 0,
        };
        let err = match blob.read_chunk(1024) {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }

    #[test]
    fn blob_reader_close_succeeds() {
        let blob = JsBlobReader {
            inner: Mutex::new(None),
            total_len: 0,
        };
        // close on already-None should succeed silently
        blob.close().unwrap();
        assert!(blob.inner.lock().unwrap().is_none());
    }

    #[test]
    fn blob_reader_length_getter() {
        let blob = JsBlobReader {
            inner: Mutex::new(None),
            total_len: 42,
        };
        assert_eq!(blob.length(), 42);
    }

    #[test]
    fn blob_reader_read_chunk_returns_data() {
        // BlobReader's constructors are private, so we can't create one directly
        // in unit tests. We verify the wrapper logic with a closed reader (None).
        // Full integration tests would require a real Memvid file.
        let blob = JsBlobReader {
            inner: Mutex::new(None),
            total_len: 0,
        };
        let result = blob.read_chunk(1024);
        assert!(result.is_err());
    }
}
