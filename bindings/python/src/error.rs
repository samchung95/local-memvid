use std::panic::{AssertUnwindSafe, catch_unwind};

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(
    memvid,
    MemvidError,
    PyException,
    "Error raised by memvid operations."
);

/// Map a `memvid_core::MemvidError` variant to a string error code.
fn error_code(e: &memvid_core::MemvidError) -> &'static str {
    use memvid_core::MemvidError::*;
    match e {
        Io { .. } => "IO",
        Encode(_) => "ENCODE",
        Decode(_) => "DECODE",
        Lock(_) => "LOCK",
        Locked(_) => "LOCKED",
        ChecksumMismatch { .. } => "CHECKSUM_MISMATCH",
        InvalidHeader { .. } => "INVALID_HEADER",
        EncryptedFile { .. } => "ENCRYPTED_FILE",
        InvalidToc { .. } => "INVALID_TOC",
        InvalidTimeIndex { .. } => "INVALID_TIME_INDEX",
        InvalidSketchTrack { .. } => "INVALID_SKETCH_TRACK",
        #[cfg(feature = "temporal_track")]
        InvalidTemporalTrack { .. } => "INVALID_TEMPORAL_TRACK",
        InvalidLogicMesh { .. } => "INVALID_LOGIC_MESH",
        LogicMeshNotEnabled => "LOGIC_MESH_NOT_ENABLED",
        NerModelNotAvailable { .. } => "NER_MODEL_NOT_AVAILABLE",
        InvalidTier => "INVALID_TIER",
        LexNotEnabled => "LEX_NOT_ENABLED",
        VecNotEnabled => "VEC_NOT_ENABLED",
        ClipNotEnabled => "CLIP_NOT_ENABLED",
        VecDimensionMismatch { .. } => "VEC_DIMENSION_MISMATCH",
        AuxiliaryFileDetected { .. } => "AUXILIARY_FILE_DETECTED",
        WalCorruption { .. } => "WAL_CORRUPTION",
        ManifestWalCorrupted { .. } => "MANIFEST_WAL_CORRUPTED",
        CheckpointFailed { .. } => "CHECKPOINT_FAILED",
        TicketSequence { .. } => "TICKET_SEQUENCE",
        TicketRequired { .. } => "TICKET_REQUIRED",
        CapacityExceeded { .. } => "CAPACITY_EXCEEDED",
        ApiKeyRequired { .. } => "API_KEY_REQUIRED",
        MemoryAlreadyBound { .. } => "MEMORY_ALREADY_BOUND",
        RequiresSealed => "REQUIRES_SEALED",
        RequiresOpen => "REQUIRES_OPEN",
        DoctorNoOp => "DOCTOR_NO_OP",
        Doctor { .. } => "DOCTOR",
        FeatureUnavailable { .. } => "FEATURE_UNAVAILABLE",
        InvalidCursor { .. } => "INVALID_CURSOR",
        InvalidFrame { .. } => "INVALID_FRAME",
        FrameNotFound { .. } => "FRAME_NOT_FOUND",
        FrameNotFoundByUri { .. } => "FRAME_NOT_FOUND_BY_URI",
        TicketSignatureInvalid { .. } => "TICKET_SIGNATURE_INVALID",
        ModelSignatureInvalid { .. } => "MODEL_SIGNATURE_INVALID",
        ModelManifestInvalid { .. } => "MODEL_MANIFEST_INVALID",
        ModelIntegrity { .. } => "MODEL_INTEGRITY",
        ExtractionFailed { .. } => "EXTRACTION_FAILED",
        EmbeddingFailed { .. } => "EMBEDDING_FAILED",
        RerankFailed { .. } => "RERANK_FAILED",
        ModelMismatch { .. } => "MODEL_MISMATCH",
        InvalidQuery { .. } => "INVALID_QUERY",
        Tantivy { .. } => "TANTIVY",
        TableExtraction { .. } => "TABLE_EXTRACTION",
        SchemaValidation { .. } => "SCHEMA_VALIDATION",
    }
}

/// Extract an optional file path from a `memvid_core::MemvidError`.
fn error_path(e: &memvid_core::MemvidError) -> Option<String> {
    use memvid_core::MemvidError::*;
    match e {
        Io { path: Some(p), .. } => Some(p.to_string_lossy().into_owned()),
        Locked(locked) => Some(locked.file.to_string_lossy().into_owned()),
        EncryptedFile { path, .. } => Some(path.to_string_lossy().into_owned()),
        AuxiliaryFileDetected { path } => Some(path.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// Build a `PyErr` from message, code, and optional path, setting attributes on
/// the exception instance so Python callers can access `.message`, `.code`, `.path`.
fn build_pyerr(py: Python<'_>, message: String, code: &str, path: Option<String>) -> PyErr {
    let err = PyErr::new::<MemvidError, _>((message.clone(),));
    {
        let val = err.value(py);
        let _ = val.setattr("message", &message);
        let _ = val.setattr("code", code);
        let _ = val.setattr("path", path);
    }
    err
}

/// Convert a `memvid_core::MemvidError` into a Python `MemvidError` exception
/// with `.message`, `.code` (string), and optional `.path` attributes.
pub fn from_memvid_error(py: Python<'_>, e: memvid_core::MemvidError) -> PyErr {
    let code = error_code(&e);
    let message = e.to_string();
    let path = error_path(&e);
    build_pyerr(py, message, code, path)
}

/// Run a closure, catching Rust panics at the PyO3 boundary and converting
/// them to `MemvidError` with code `INTERNAL`.
pub fn catch_panic<F, T>(py: Python<'_>, f: F) -> PyResult<T>
where
    F: FnOnce() -> PyResult<T>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(panic) => {
            let detail = if let Some(s) = panic.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown cause".to_string()
            };
            let message = format!("Internal panic: {detail}");
            Err(build_pyerr(py, message, "INTERNAL", None))
        }
    }
}

/// Build a `MemvidError` with code `CLOSED` for use when the handle has been
/// closed or the inner lock is poisoned.
pub fn closed_error(py: Python<'_>, detail: &str) -> PyErr {
    build_pyerr(py, detail.to_string(), "CLOSED", None)
}

/// Register the `MemvidError` exception class on the Python module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("MemvidError", m.py().get_type::<MemvidError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_io_error_maps_to_io_code() {
        let err = memvid_core::MemvidError::Io {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
            path: Some(PathBuf::from("/tmp/test.mv2")),
        };
        assert_eq!(error_code(&err), "IO");
        assert_eq!(error_path(&err), Some("/tmp/test.mv2".to_string()));
    }

    #[test]
    fn test_checksum_mismatch_maps_to_code() {
        let err = memvid_core::MemvidError::ChecksumMismatch { context: "header" };
        assert_eq!(error_code(&err), "CHECKSUM_MISMATCH");
        assert_eq!(error_path(&err), None);
    }

    #[test]
    fn test_capacity_exceeded_maps_to_code() {
        let err = memvid_core::MemvidError::CapacityExceeded {
            current: 100,
            limit: 50,
            required: 150,
        };
        assert_eq!(error_code(&err), "CAPACITY_EXCEEDED");
        assert_eq!(error_path(&err), None);
    }

    #[test]
    fn test_encrypted_file_maps_to_code_with_path() {
        let err = memvid_core::MemvidError::EncryptedFile {
            path: PathBuf::from("/secret.mv2"),
            hint: "decrypt first".to_string(),
        };
        assert_eq!(error_code(&err), "ENCRYPTED_FILE");
        assert_eq!(error_path(&err), Some("/secret.mv2".to_string()));
    }

    #[test]
    fn test_wal_corruption_maps_to_code() {
        let err = memvid_core::MemvidError::WalCorruption {
            offset: 42,
            reason: "bad magic".into(),
        };
        assert_eq!(error_code(&err), "WAL_CORRUPTION");
        assert_eq!(error_path(&err), None);
    }

    #[test]
    fn test_lex_not_enabled_maps_to_code() {
        let err = memvid_core::MemvidError::LexNotEnabled;
        assert_eq!(error_code(&err), "LEX_NOT_ENABLED");
    }

    #[test]
    fn test_vec_not_enabled_maps_to_code() {
        let err = memvid_core::MemvidError::VecNotEnabled;
        assert_eq!(error_code(&err), "VEC_NOT_ENABLED");
    }

    #[test]
    fn test_frame_not_found_maps_to_code() {
        let err = memvid_core::MemvidError::FrameNotFound { frame_id: 99 };
        assert_eq!(error_code(&err), "FRAME_NOT_FOUND");
        assert_eq!(error_path(&err), None);
    }
}
