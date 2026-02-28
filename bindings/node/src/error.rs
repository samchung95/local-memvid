use memvid_core::MemvidError;

/// Holds the decomposed error information before conversion to a NAPI error.
///
/// On the JavaScript side the resulting `Error` carries:
/// - `message` – human-readable description
/// - embedded `[CODE]` prefix for programmatic matching
/// - optional path context when the error is file-related
pub struct NapiMemvidError {
    pub code: &'static str,
    pub message: String,
    pub path: Option<String>,
}

/// Map a [`MemvidError`] variant to a stable string error code.
pub fn error_code(err: &MemvidError) -> &'static str {
    match err {
        // I/O & serialization
        MemvidError::Io { .. } => "IO",
        MemvidError::Encode(..) => "ENCODE",
        MemvidError::Decode(..) => "DECODE",

        // Locking
        MemvidError::Lock(..) => "LOCKED",
        MemvidError::Locked(..) => "LOCKED",

        // Validation
        MemvidError::ChecksumMismatch { .. } => "CHECKSUM_MISMATCH",
        MemvidError::InvalidHeader { .. } => "INVALID_HEADER",
        MemvidError::InvalidToc { .. } => "INVALID_TOC",
        MemvidError::InvalidTimeIndex { .. } => "INVALID_TIME_INDEX",
        MemvidError::InvalidSketchTrack { .. } => "INVALID_SKETCH_TRACK",
        #[cfg(feature = "temporal_track")]
        MemvidError::InvalidTemporalTrack { .. } => "INVALID_TEMPORAL_TRACK",
        MemvidError::InvalidLogicMesh { .. } => "INVALID_LOGIC_MESH",

        // Encryption & files
        MemvidError::EncryptedFile { .. } => "ENCRYPTED_FILE",
        MemvidError::AuxiliaryFileDetected { .. } => "AUXILIARY_FILE_DETECTED",

        // Feature availability
        MemvidError::LogicMeshNotEnabled => "LOGIC_MESH_NOT_ENABLED",
        MemvidError::NerModelNotAvailable { .. } => "NER_MODEL_NOT_AVAILABLE",
        MemvidError::InvalidTier => "INVALID_TIER",
        MemvidError::LexNotEnabled => "LEX_NOT_ENABLED",
        MemvidError::VecNotEnabled => "VEC_NOT_ENABLED",
        MemvidError::ClipNotEnabled => "CLIP_NOT_ENABLED",
        MemvidError::FeatureUnavailable { .. } => "FEATURE_UNAVAILABLE",

        // Vector indexing
        MemvidError::VecDimensionMismatch { .. } => "VEC_DIMENSION_MISMATCH",

        // WAL
        MemvidError::WalCorruption { .. } => "WAL_CORRUPTION",
        MemvidError::ManifestWalCorrupted { .. } => "WAL_CORRUPTION",
        MemvidError::CheckpointFailed { .. } => "CHECKPOINT_FAILED",

        // Tickets
        MemvidError::TicketSequence { .. } => "TICKET_SEQUENCE",
        MemvidError::TicketRequired { .. } => "TICKET_REQUIRED",
        MemvidError::TicketSignatureInvalid { .. } => "TICKET_SIGNATURE_INVALID",

        // Capacity / API key
        MemvidError::CapacityExceeded { .. } => "CAPACITY_EXCEEDED",
        MemvidError::ApiKeyRequired { .. } => "API_KEY_REQUIRED",

        // Memory binding
        MemvidError::MemoryAlreadyBound { .. } => "MEMORY_ALREADY_BOUND",
        MemvidError::RequiresSealed => "REQUIRES_SEALED",
        MemvidError::RequiresOpen => "REQUIRES_OPEN",

        // Doctor
        MemvidError::DoctorNoOp => "DOCTOR_NO_OP",
        MemvidError::Doctor { .. } => "DOCTOR",

        // Search / query
        MemvidError::InvalidCursor { .. } => "INVALID_CURSOR",
        MemvidError::InvalidQuery { .. } => "INVALID_QUERY",

        // Frame
        MemvidError::InvalidFrame { .. } => "INVALID_FRAME",
        MemvidError::FrameNotFound { .. } => "FRAME_NOT_FOUND",
        MemvidError::FrameNotFoundByUri { .. } => "FRAME_NOT_FOUND",

        // Model / AI
        MemvidError::ModelSignatureInvalid { .. } => "MODEL_SIGNATURE_INVALID",
        MemvidError::ModelManifestInvalid { .. } => "MODEL_MANIFEST_INVALID",
        MemvidError::ModelIntegrity { .. } => "MODEL_INTEGRITY",
        MemvidError::ModelMismatch { .. } => "MODEL_MISMATCH",

        // Processing
        MemvidError::ExtractionFailed { .. } => "EXTRACTION_FAILED",
        MemvidError::EmbeddingFailed { .. } => "EMBEDDING_FAILED",
        MemvidError::RerankFailed { .. } => "RERANK_FAILED",
        MemvidError::Tantivy { .. } => "TANTIVY",
        MemvidError::TableExtraction { .. } => "TABLE_EXTRACTION",
        MemvidError::SchemaValidation { .. } => "SCHEMA_VALIDATION",
    }
}

/// Extract the file path from error variants that carry one.
fn error_path(err: &MemvidError) -> Option<String> {
    match err {
        MemvidError::Io { path: Some(p), .. } => Some(p.display().to_string()),
        MemvidError::EncryptedFile { path, .. } => Some(path.display().to_string()),
        MemvidError::AuxiliaryFileDetected { path } => Some(path.display().to_string()),
        MemvidError::Locked(locked) => Some(locked.file.display().to_string()),
        _ => None,
    }
}

impl From<MemvidError> for NapiMemvidError {
    fn from(err: MemvidError) -> Self {
        let code = error_code(&err);
        let path = error_path(&err);
        let message = err.to_string();
        Self {
            code,
            message,
            path,
        }
    }
}

/// Convert a [`MemvidError`] into a [`napi::Error`] for throwing across the
/// NAPI boundary.
///
/// The JS `Error.message` is formatted as `[CODE] description` so callers can
/// match on the prefix programmatically, while the full description remains
/// human-readable.
pub fn from_memvid_error(err: MemvidError) -> napi::Error {
    let napi_err = NapiMemvidError::from(err);
    let msg = if let Some(ref p) = napi_err.path {
        format!("[{}] {} (path: {})", napi_err.code, napi_err.message, p)
    } else {
        format!("[{}] {}", napi_err.code, napi_err.message)
    };
    napi::Error::new(napi::Status::GenericFailure, msg)
}

/// Run `f` inside [`std::panic::catch_unwind`], converting both
/// [`MemvidError`] results and unexpected panics into [`napi::Error`].
///
/// Panics are surfaced with the `INTERNAL` error code.
pub fn catch_unwind_napi<F, T>(f: F) -> napi::Result<T>
where
    F: FnOnce() -> std::result::Result<T, MemvidError> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(Ok(val)) => Ok(val),
        Ok(Err(err)) => Err(from_memvid_error(err)),
        Err(panic_payload) => {
            let reason = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!("[INTERNAL] Internal error: {reason}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn io_error_maps_to_io_code() {
        let err = MemvidError::Io {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "file missing"),
            path: Some(PathBuf::from("/tmp/test.mv2")),
        };
        assert_eq!(error_code(&err), "IO");

        let napi_err = NapiMemvidError::from(err);
        assert_eq!(napi_err.code, "IO");
        assert_eq!(napi_err.path.as_deref(), Some("/tmp/test.mv2"));
    }

    #[test]
    fn checksum_mismatch_maps_correctly() {
        let err = MemvidError::ChecksumMismatch { context: "footer" };
        assert_eq!(error_code(&err), "CHECKSUM_MISMATCH");

        let napi_err = from_memvid_error(err);
        assert!(napi_err.to_string().contains("[CHECKSUM_MISMATCH]"));
    }

    #[test]
    fn capacity_exceeded_maps_correctly() {
        let err = MemvidError::CapacityExceeded {
            current: 100,
            limit: 50,
            required: 200,
        };
        assert_eq!(error_code(&err), "CAPACITY_EXCEEDED");

        let napi_err = from_memvid_error(err);
        let msg = napi_err.to_string();
        assert!(msg.contains("[CAPACITY_EXCEEDED]"));
        assert!(msg.contains("100"));
    }

    #[test]
    fn lex_not_enabled_maps_correctly() {
        let err = MemvidError::LexNotEnabled;
        assert_eq!(error_code(&err), "LEX_NOT_ENABLED");
    }

    #[test]
    fn vec_not_enabled_maps_correctly() {
        let err = MemvidError::VecNotEnabled;
        assert_eq!(error_code(&err), "VEC_NOT_ENABLED");
    }

    #[test]
    fn encrypted_file_includes_path() {
        let err = MemvidError::EncryptedFile {
            path: PathBuf::from("/secret/data.mv2"),
            hint: "use --decrypt".into(),
        };
        assert_eq!(error_code(&err), "ENCRYPTED_FILE");

        let napi_err = NapiMemvidError::from(err);
        assert_eq!(napi_err.path.as_deref(), Some("/secret/data.mv2"));
    }

    #[test]
    fn wal_corruption_maps_correctly() {
        let err = MemvidError::WalCorruption {
            offset: 42,
            reason: "bad checksum".into(),
        };
        assert_eq!(error_code(&err), "WAL_CORRUPTION");
    }

    #[test]
    fn catch_unwind_converts_panic_to_internal() {
        let result: napi::Result<()> = catch_unwind_napi(|| {
            panic!("test panic");
        });
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("[INTERNAL]"));
        assert!(msg.contains("test panic"));
    }

    #[test]
    fn catch_unwind_passes_through_ok() {
        let result = catch_unwind_napi(|| Ok::<_, MemvidError>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn catch_unwind_converts_memvid_error() {
        let result: napi::Result<()> = catch_unwind_napi(|| Err(MemvidError::ClipNotEnabled));
        let err = result.unwrap_err();
        assert!(err.to_string().contains("[CLIP_NOT_ENABLED]"));
    }
}
