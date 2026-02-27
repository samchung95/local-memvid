use pyo3::prelude::*;
use pyo3::types::PyDict;

use memvid_core::{DocumentFormat, ReaderDiagnostics, ReaderHint, ReaderOutput, ReaderRegistry};
use serde_json::Value;

use crate::error;
use crate::lifecycle::{PyMemvid, guard_memvid};

// ---------------------------------------------------------------------------
// serde_json::Value → Python conversion
// ---------------------------------------------------------------------------

fn value_to_py(py: Python<'_>, val: &Value) -> PyObject {
    match val {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_pyobject(py).unwrap().to_owned().into_any().unbind(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py).unwrap().into_any().unbind()
            } else if let Some(f) = n.as_f64() {
                f.into_pyobject(py).unwrap().into_any().unbind()
            } else {
                py.None()
            }
        }
        Value::String(s) => s.into_pyobject(py).unwrap().into_any().unbind(),
        Value::Array(arr) => {
            let items: Vec<PyObject> = arr.iter().map(|v| value_to_py(py, v)).collect();
            items.into_pyobject(py).unwrap().into_any().unbind()
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, value_to_py(py, v)).unwrap();
            }
            dict.into_pyobject(py).unwrap().into_any().unbind()
        }
    }
}

// ---------------------------------------------------------------------------
// Python wrapper types
// ---------------------------------------------------------------------------

/// Diagnostics from a document extraction.
#[pyclass(name = "ReaderDiagnostics", frozen)]
pub struct PyReaderDiagnostics {
    #[pyo3(get)]
    pub warnings: Vec<String>,
    #[pyo3(get)]
    pub fallback: bool,
    #[pyo3(get)]
    pub duration_ms: Option<u64>,
    #[pyo3(get)]
    pub pages_processed: Option<u32>,
}

#[pymethods]
impl PyReaderDiagnostics {
    fn __repr__(&self) -> String {
        format!(
            "ReaderDiagnostics(fallback={}, warnings={}, duration_ms={:?}, pages_processed={:?})",
            self.fallback,
            self.warnings.len(),
            self.duration_ms,
            self.pages_processed,
        )
    }
}

impl From<&ReaderDiagnostics> for PyReaderDiagnostics {
    fn from(d: &ReaderDiagnostics) -> Self {
        Self {
            warnings: d.warnings.clone(),
            fallback: d.fallback,
            duration_ms: d.duration_ms,
            pages_processed: d.pages_processed,
        }
    }
}

/// Output from a document reader extraction.
#[pyclass(name = "ReaderOutput", frozen)]
pub struct PyReaderOutput {
    #[pyo3(get)]
    pub text: Option<String>,
    #[pyo3(get)]
    pub format: Option<String>,
    #[pyo3(get)]
    pub reader_name: String,
    raw_metadata: Value,
    raw_diagnostics: ReaderDiagnostics,
}

#[pymethods]
impl PyReaderOutput {
    /// Document metadata as a Python dict.
    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyObject {
        value_to_py(py, &self.raw_metadata)
    }

    /// Extraction diagnostics.
    #[getter]
    fn diagnostics(&self) -> PyReaderDiagnostics {
        PyReaderDiagnostics::from(&self.raw_diagnostics)
    }

    fn __repr__(&self) -> String {
        format!(
            "ReaderOutput(reader_name={:?}, format={:?}, text_len={})",
            self.reader_name,
            self.format,
            self.text.as_ref().map_or(0, |t| t.len()),
        )
    }
}

impl From<ReaderOutput> for PyReaderOutput {
    fn from(o: ReaderOutput) -> Self {
        Self {
            text: o.document.text,
            format: o.document.mime_type,
            reader_name: o.reader_name,
            raw_metadata: o.document.metadata,
            raw_diagnostics: o.diagnostics,
        }
    }
}

// ---------------------------------------------------------------------------
// PyReaderRegistry
// ---------------------------------------------------------------------------

/// Registry of document readers for extracting text from files.
///
/// Construct with `ReaderRegistry()` to get all built-in readers (PDF, DOCX,
/// XLSX, XLS, PPTX, plaintext).
#[pyclass(name = "ReaderRegistry")]
pub struct PyReaderRegistry {
    inner: ReaderRegistry,
}

#[pymethods]
impl PyReaderRegistry {
    /// Create a new registry with all built-in readers.
    #[new]
    fn new() -> Self {
        Self {
            inner: ReaderRegistry::default(),
        }
    }

    /// Extract text and metadata from raw document bytes.
    ///
    /// Provide `filename` and/or `mime_type` to help identify the format.
    /// Returns `ReaderOutput` with extracted text, metadata, and diagnostics.
    /// Raises `MemvidError` if no reader supports the given format or extraction fails.
    #[pyo3(signature = (data, *, filename=None, mime_type=None))]
    fn extract(
        &self,
        py: Python<'_>,
        data: &[u8],
        filename: Option<&str>,
        mime_type: Option<&str>,
    ) -> PyResult<PyReaderOutput> {
        error::catch_panic(py, || {
            let format = guess_format(filename, mime_type);
            let hint = ReaderHint::new(mime_type, format)
                .with_uri(filename)
                .with_magic(if data.len() >= 8 {
                    Some(&data[..8])
                } else if !data.is_empty() {
                    Some(data)
                } else {
                    None
                });

            let reader = self.inner.find_reader(&hint).ok_or_else(|| {
                error::from_memvid_error(
                    py,
                    memvid_core::MemvidError::Io {
                        source: std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            format!(
                                "no reader found for filename={:?}, mime_type={:?}",
                                filename, mime_type
                            ),
                        ),
                        path: None,
                    },
                )
            })?;

            let output = reader
                .extract(data, &hint)
                .map_err(|e| error::from_memvid_error(py, e))?;

            Ok(PyReaderOutput::from(output))
        })
    }

    fn __repr__(&self) -> String {
        format!("ReaderRegistry(readers={})", self.inner.readers().len())
    }
}

// ---------------------------------------------------------------------------
// preview_chunks on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    /// Preview how document bytes would be chunked during ingestion.
    ///
    /// Returns a list of text chunks, or None if the document is too small to chunk.
    fn preview_chunks(&self, py: Python<'_>, data: &[u8]) -> PyResult<Option<Vec<String>>> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            Ok(mv.preview_chunks(data))
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Best-effort guess of DocumentFormat from filename extension or MIME type.
fn guess_format(filename: Option<&str>, mime_type: Option<&str>) -> Option<DocumentFormat> {
    // Try extension first
    if let Some(name) = filename {
        let lower = name.to_lowercase();
        if lower.ends_with(".pdf") {
            return Some(DocumentFormat::Pdf);
        } else if lower.ends_with(".docx") {
            return Some(DocumentFormat::Docx);
        } else if lower.ends_with(".xlsx") {
            return Some(DocumentFormat::Xlsx);
        } else if lower.ends_with(".xls") {
            return Some(DocumentFormat::Xls);
        } else if lower.ends_with(".pptx") {
            return Some(DocumentFormat::Pptx);
        } else if lower.ends_with(".txt") {
            return Some(DocumentFormat::PlainText);
        } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
            return Some(DocumentFormat::Markdown);
        } else if lower.ends_with(".html") || lower.ends_with(".htm") {
            return Some(DocumentFormat::Html);
        } else if lower.ends_with(".jsonl") {
            return Some(DocumentFormat::Jsonl);
        }
    }

    // Fall back to MIME type
    if let Some(mime) = mime_type {
        let lower = mime.to_lowercase();
        return match lower.as_str() {
            "application/pdf" => Some(DocumentFormat::Pdf),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Some(DocumentFormat::Docx)
            }
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                Some(DocumentFormat::Xlsx)
            }
            "application/vnd.ms-excel" => Some(DocumentFormat::Xls),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
                Some(DocumentFormat::Pptx)
            }
            "text/plain" => Some(DocumentFormat::PlainText),
            "text/markdown" => Some(DocumentFormat::Markdown),
            "text/html" => Some(DocumentFormat::Html),
            _ => None,
        };
    }

    None
}

/// Register reader-related classes on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyReaderDiagnostics>()?;
    m.add_class::<PyReaderOutput>()?;
    m.add_class::<PyReaderRegistry>()?;
    Ok(())
}
