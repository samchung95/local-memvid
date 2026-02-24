use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

use memvid_core::{DocumentFormat, ReaderHint, ReaderRegistry};

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// JS types
// ---------------------------------------------------------------------------

/// Hint object passed to `extract` so the registry can pick the right reader.
#[napi(object)]
pub struct JsReaderHint {
    /// Original filename (e.g. "report.pdf") — used to infer format.
    pub filename: Option<String>,
    /// MIME type (e.g. "application/pdf").
    pub mime_type: Option<String>,
}

/// Diagnostics about the reader that produced the output.
#[napi(object)]
pub struct JsReaderDiagnostics {
    /// Warnings emitted during extraction.
    pub warnings: Vec<String>,
    /// Whether a fallback reader was used.
    pub fallback: bool,
    /// Extraction duration in milliseconds.
    pub duration_ms: Option<f64>,
    /// Number of pages processed (for PDFs, etc.).
    pub pages_processed: Option<u32>,
}

/// Output from a document reader extraction.
#[napi(object)]
pub struct JsReaderOutput {
    /// Extracted plaintext (empty string if document is binary/image-only).
    pub text: String,
    /// Arbitrary metadata as key-value pairs.
    pub metadata: HashMap<String, String>,
    /// Detected or declared format label (e.g. "pdf", "docx", "text").
    pub format: String,
    /// Name of the reader that produced this output (e.g. "pdf", "docx").
    pub reader_name: String,
    /// Diagnostics about the extraction.
    pub diagnostics: JsReaderDiagnostics,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Infer a `DocumentFormat` from a filename extension.
fn format_from_filename(filename: &str) -> Option<DocumentFormat> {
    let ext = filename.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => Some(DocumentFormat::Pdf),
        "docx" => Some(DocumentFormat::Docx),
        "xlsx" => Some(DocumentFormat::Xlsx),
        "xls" => Some(DocumentFormat::Xls),
        "pptx" => Some(DocumentFormat::Pptx),
        "txt" => Some(DocumentFormat::PlainText),
        "md" | "markdown" => Some(DocumentFormat::Markdown),
        "html" | "htm" => Some(DocumentFormat::Html),
        "jsonl" => Some(DocumentFormat::Jsonl),
        _ => None,
    }
}

/// Build a `ReaderHint` from the JS hint object + leading bytes.
fn build_hint<'a>(
    js: &'a JsReaderHint,
    magic: &'a [u8],
) -> ReaderHint<'a> {
    let format = js
        .filename
        .as_deref()
        .and_then(format_from_filename);
    let mime = js.mime_type.as_deref();
    ReaderHint::new(mime, format)
        .with_uri(js.filename.as_deref())
        .with_magic(Some(magic))
}

/// Convert a `serde_json::Value` metadata object to a flat `HashMap<String, String>`.
fn metadata_to_map(value: &serde_json::Value) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            let s = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            map.insert(k.clone(), s);
        }
    }
    map
}

/// Convert a Rust `ReaderOutput` into a `JsReaderOutput`.
fn from_reader_output(out: memvid_core::ReaderOutput) -> JsReaderOutput {
    let text = out.document.text.unwrap_or_default();
    let metadata = metadata_to_map(&out.document.metadata);
    let format = out
        .document
        .mime_type
        .unwrap_or_else(|| "unknown".to_string());
    JsReaderOutput {
        text,
        metadata,
        format,
        reader_name: out.reader_name,
        diagnostics: JsReaderDiagnostics {
            warnings: out.diagnostics.warnings,
            fallback: out.diagnostics.fallback,
            duration_ms: out.diagnostics.duration_ms.map(|v| v as f64),
            pages_processed: out.diagnostics.pages_processed,
        },
    }
}

// ---------------------------------------------------------------------------
// JsReaderRegistry — wraps ReaderRegistry
// ---------------------------------------------------------------------------

/// Document reader registry that can extract text from PDFs, DOCX, XLSX, etc.
///
/// Create with `JsReaderRegistry.newDefault()` to include all built-in readers.
#[napi]
pub struct JsReaderRegistry {
    inner: Arc<Mutex<ReaderRegistry>>,
}

#[napi]
impl JsReaderRegistry {
    /// Create a new registry pre-loaded with all built-in readers
    /// (PDF, DOCX, XLSX, XLS, PPTX, plaintext fallback).
    #[napi(factory, js_name = "newDefault")]
    pub fn new_default() -> JsReaderRegistry {
        JsReaderRegistry {
            inner: Arc::new(Mutex::new(ReaderRegistry::default())),
        }
    }

    /// Extract text and metadata from a document buffer (sync).
    ///
    /// The `hint` object helps the registry pick the correct reader —
    /// provide `filename` and/or `mimeType` for best results.
    #[napi(js_name = "extractSync")]
    pub fn extract_sync(
        &self,
        data: Buffer,
        hint: Option<JsReaderHint>,
    ) -> napi::Result<JsReaderOutput> {
        let hint = hint.unwrap_or(JsReaderHint {
            filename: None,
            mime_type: None,
        });
        let bytes: &[u8] = &data;
        let magic = if bytes.len() >= 16 { &bytes[..16] } else { bytes };
        let reader_hint = build_hint(&hint, magic);

        let registry = self.inner.lock().map_err(|_| {
            napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
        })?;

        let reader = registry.find_reader(&reader_hint).ok_or_else(|| {
            napi::Error::new(
                napi::Status::GenericFailure,
                "No reader found for the given document",
            )
        })?;

        let output = reader.extract(bytes, &reader_hint).map_err(from_memvid_error)?;
        Ok(from_reader_output(output))
    }

    /// Extract text and metadata from a document buffer (async).
    #[napi(js_name = "extract")]
    pub async fn extract(
        &self,
        data: Buffer,
        hint: Option<JsReaderHint>,
    ) -> napi::Result<JsReaderOutput> {
        let inner = Arc::clone(&self.inner);
        let bytes = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let hint = hint.unwrap_or(JsReaderHint {
                filename: None,
                mime_type: None,
            });
            let magic = if bytes.len() >= 16 {
                &bytes[..16]
            } else {
                &bytes
            };
            let reader_hint = build_hint(&hint, magic);

            let registry = inner.lock().map_err(|_| {
                napi::Error::new(napi::Status::GenericFailure, "[INTERNAL] Mutex poisoned")
            })?;

            let reader = registry.find_reader(&reader_hint).ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "No reader found for the given document",
                )
            })?;

            let output = reader.extract(&bytes, &reader_hint).map_err(from_memvid_error)?;
            Ok(from_reader_output(output))
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }
}

// ---------------------------------------------------------------------------
// previewChunks on JsMemvid
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Preview how a document will be chunked before embedding (sync).
    ///
    /// Returns an array of chunk strings, or `null` if the document is too
    /// small to chunk (< ~2,400 chars after normalization).
    #[napi(js_name = "previewChunksSync")]
    pub fn preview_chunks_sync(&self, data: Buffer) -> napi::Result<Option<Vec<String>>> {
        let guard = guard_memvid!(self);
        Ok(guard.as_ref().unwrap().preview_chunks(&data))
    }

    /// Preview how a document will be chunked before embedding (async).
    #[napi(js_name = "previewChunks")]
    pub async fn preview_chunks(&self, data: Buffer) -> napi::Result<Option<Vec<String>>> {
        let inner = Arc::clone(&self.inner);
        let bytes = data.to_vec();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            Ok(guard.as_ref().unwrap().preview_chunks(&bytes))
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
    fn format_from_filename_pdf() {
        assert!(matches!(
            format_from_filename("report.pdf"),
            Some(DocumentFormat::Pdf)
        ));
    }

    #[test]
    fn format_from_filename_docx() {
        assert!(matches!(
            format_from_filename("doc.docx"),
            Some(DocumentFormat::Docx)
        ));
    }

    #[test]
    fn format_from_filename_xlsx() {
        assert!(matches!(
            format_from_filename("data.xlsx"),
            Some(DocumentFormat::Xlsx)
        ));
    }

    #[test]
    fn format_from_filename_unknown() {
        assert!(format_from_filename("file.xyz").is_none());
    }

    #[test]
    fn format_from_filename_case_insensitive() {
        assert!(matches!(
            format_from_filename("REPORT.PDF"),
            Some(DocumentFormat::Pdf)
        ));
    }

    #[test]
    fn metadata_to_map_handles_object() {
        let val = serde_json::json!({
            "Content-Type": "text/plain",
            "count": 42
        });
        let map = metadata_to_map(&val);
        assert_eq!(map.get("Content-Type").unwrap(), "text/plain");
        assert_eq!(map.get("count").unwrap(), "42");
    }

    #[test]
    fn metadata_to_map_handles_non_object() {
        let val = serde_json::json!("just a string");
        let map = metadata_to_map(&val);
        assert!(map.is_empty());
    }

    #[test]
    fn new_default_creates_registry() {
        let reg = JsReaderRegistry::new_default();
        // Should have the Mutex with a registry inside
        let guard = reg.inner.lock().unwrap();
        // Default registry has 6 readers (pdf, docx, xlsx, xls, pptx, passthrough)
        assert!(guard.readers().len() >= 5);
    }

    #[test]
    fn extract_sync_plain_text() {
        let reg = JsReaderRegistry::new_default();
        let data = Buffer::from("Hello, world! This is a test document.".as_bytes());
        let hint = JsReaderHint {
            filename: Some("test.txt".to_string()),
            mime_type: Some("text/plain".to_string()),
        };
        let result = reg.extract_sync(data, Some(hint));
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.text.contains("Hello, world!"));
    }

    #[test]
    fn extract_sync_no_reader_for_hint() {
        let reg = JsReaderRegistry {
            inner: Arc::new(Mutex::new(ReaderRegistry::new())),
        };
        let data = Buffer::from(&[0u8; 4][..]);
        let hint = JsReaderHint {
            filename: Some("file.xyz".to_string()),
            mime_type: Some("application/x-unknown-binary".to_string()),
        };
        let result = reg.extract_sync(data, Some(hint));
        match result {
            Err(e) => assert!(e.to_string().contains("No reader found")),
            Ok(_) => panic!("expected Err"),
        }
    }

    #[test]
    fn closed_instance_rejects_preview_chunks() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let data = Buffer::from("hello".as_bytes());
        let result = js.preview_chunks_sync(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("[CLOSED]"));
    }

    #[test]
    fn from_reader_output_converts_fields() {
        use memvid_core::reader::ReaderDiagnostics;

        let doc = memvid_core::ExtractedDocument {
            text: Some("extracted text".to_string()),
            metadata: serde_json::json!({"key": "value"}),
            mime_type: Some("text/plain".to_string()),
        };
        let diag = ReaderDiagnostics {
            warnings: vec!["warn1".to_string()],
            fallback: true,
            extra_metadata: serde_json::Value::Null,
            duration_ms: Some(42),
            pages_processed: Some(3),
        };
        let output = memvid_core::ReaderOutput::new(doc, "test_reader").with_diagnostics(diag);
        let js = from_reader_output(output);
        assert_eq!(js.text, "extracted text");
        assert_eq!(js.reader_name, "test_reader");
        assert_eq!(js.format, "text/plain");
        assert_eq!(js.metadata.get("key").unwrap(), "value");
        assert_eq!(js.diagnostics.warnings, vec!["warn1"]);
        assert!(js.diagnostics.fallback);
        assert_eq!(js.diagnostics.duration_ms, Some(42.0));
        assert_eq!(js.diagnostics.pages_processed, Some(3));
    }
}
