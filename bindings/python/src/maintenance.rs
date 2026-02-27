use pyo3::prelude::*;

use crate::error;
use crate::lifecycle::{guard_memvid, PyMemvid};

// ---------------------------------------------------------------------------
// Helper: enum → snake_case string
// ---------------------------------------------------------------------------

fn verification_status_str(s: &memvid_core::types::VerificationStatus) -> &'static str {
    use memvid_core::types::VerificationStatus::*;
    match s {
        Passed => "passed",
        Failed => "failed",
        Skipped => "skipped",
    }
}

fn doctor_status_str(s: &memvid_core::types::DoctorStatus) -> &'static str {
    use memvid_core::types::DoctorStatus::*;
    match s {
        Clean => "clean",
        Healed => "healed",
        Partial => "partial",
        Failed => "failed",
        PlanOnly => "plan_only",
    }
}

fn doctor_severity_str(s: &memvid_core::types::verification::DoctorSeverity) -> &'static str {
    use memvid_core::types::verification::DoctorSeverity::*;
    match s {
        Info => "info",
        Warning => "warning",
        Error => "error",
    }
}

fn doctor_phase_kind_str(k: &memvid_core::types::verification::DoctorPhaseKind) -> &'static str {
    use memvid_core::types::verification::DoctorPhaseKind::*;
    match k {
        Probe => "probe",
        HeaderHealing => "header_healing",
        WalReplay => "wal_replay",
        IndexRebuild => "index_rebuild",
        Vacuum => "vacuum",
        Finalize => "finalize",
        Verify => "verify",
    }
}

fn doctor_phase_status_str(
    s: &memvid_core::types::verification::DoctorPhaseStatus,
) -> &'static str {
    use memvid_core::types::verification::DoctorPhaseStatus::*;
    match s {
        Skipped => "skipped",
        Executed => "executed",
        Failed => "failed",
    }
}

fn doctor_action_kind_str(
    k: &memvid_core::types::verification::DoctorActionKind,
) -> &'static str {
    use memvid_core::types::verification::DoctorActionKind::*;
    match k {
        HealHeaderPointer => "heal_header_pointer",
        HealTocChecksum => "heal_toc_checksum",
        ReplayWal => "replay_wal",
        DiscardWal => "discard_wal",
        RebuildTimeIndex => "rebuild_time_index",
        RebuildLexIndex => "rebuild_lex_index",
        RebuildVecIndex => "rebuild_vec_index",
        VacuumCompaction => "vacuum_compaction",
        RecomputeToc => "recompute_toc",
        UpdateHeader => "update_header",
        DeepVerify => "deep_verify",
        NoOp => "no_op",
    }
}

fn doctor_action_status_str(
    s: &memvid_core::types::verification::DoctorActionStatus,
) -> &'static str {
    use memvid_core::types::verification::DoctorActionStatus::*;
    match s {
        Skipped => "skipped",
        Executed => "executed",
        Failed => "failed",
    }
}

fn doctor_finding_code_str(
    c: &memvid_core::types::verification::DoctorFindingCode,
) -> &'static str {
    use memvid_core::types::verification::DoctorFindingCode::*;
    match c {
        HeaderFooterOffsetMismatch => "header_footer_offset_mismatch",
        HeaderTocChecksumMismatch => "header_toc_checksum_mismatch",
        HeaderDecodeFailure => "header_decode_failure",
        TocDecodeFailure => "toc_decode_failure",
        TocChecksumMismatch => "toc_checksum_mismatch",
        TocOutOfBounds => "toc_out_of_bounds",
        WalHasPendingRecords => "wal_has_pending_records",
        WalSequenceAheadOfHeader => "wal_sequence_ahead_of_header",
        WalChecksumMismatch => "wal_checksum_mismatch",
        TimeIndexMissing => "time_index_missing",
        TimeIndexChecksumMismatch => "time_index_checksum_mismatch",
        TimeIndexUnsorted => "time_index_unsorted",
        LexIndexMissing => "lex_index_missing",
        LexIndexCorrupt => "lex_index_corrupt",
        VecIndexMissing => "vec_index_missing",
        VecIndexCorrupt => "vec_index_corrupt",
        TantivySnapshotMissing => "tantivy_snapshot_missing",
        TantivySnapshotCorrupt => "tantivy_snapshot_corrupt",
        MerkleMismatch => "merkle_mismatch",
        SegmentCatalogInconsistent => "segment_catalog_inconsistent",
        VacuumIncomplete => "vacuum_incomplete",
        LockContention => "lock_contention",
        UnsupportedFeature => "unsupported_feature",
        InternalError => "internal_error",
    }
}

fn tier_str(t: &memvid_core::types::Tier) -> &'static str {
    use memvid_core::types::Tier::*;
    match t {
        Free => "free",
        Dev => "dev",
        Enterprise => "enterprise",
    }
}

// ---------------------------------------------------------------------------
// VerificationCheck
// ---------------------------------------------------------------------------

#[pyclass(name = "VerificationCheck", frozen)]
pub struct PyVerificationCheck {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    details: Option<String>,
}

impl From<memvid_core::types::VerificationCheck> for PyVerificationCheck {
    fn from(c: memvid_core::types::VerificationCheck) -> Self {
        Self {
            name: c.name,
            status: verification_status_str(&c.status).to_string(),
            details: c.details,
        }
    }
}

#[pymethods]
impl PyVerificationCheck {
    fn __repr__(&self) -> String {
        format!(
            "VerificationCheck(name={:?}, status={:?})",
            self.name, self.status
        )
    }
}

// ---------------------------------------------------------------------------
// VerificationReport
// ---------------------------------------------------------------------------

#[pyclass(name = "VerificationReport", frozen)]
pub struct PyVerificationReport {
    #[pyo3(get)]
    file_path: String,
    #[pyo3(get)]
    overall_status: String,
    raw_checks: Vec<PyVerificationCheck>,
}

impl From<memvid_core::types::VerificationReport> for PyVerificationReport {
    fn from(r: memvid_core::types::VerificationReport) -> Self {
        Self {
            file_path: r.file_path.to_string_lossy().into_owned(),
            overall_status: verification_status_str(&r.overall_status).to_string(),
            raw_checks: r.checks.into_iter().map(PyVerificationCheck::from).collect(),
        }
    }
}

#[pymethods]
impl PyVerificationReport {
    #[getter]
    fn checks(&self) -> Vec<PyVerificationCheck> {
        self.raw_checks
            .iter()
            .map(|c| PyVerificationCheck {
                name: c.name.clone(),
                status: c.status.clone(),
                details: c.details.clone(),
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "VerificationReport(file_path={:?}, overall_status={:?}, checks={})",
            self.file_path,
            self.overall_status,
            self.raw_checks.len()
        )
    }
}

// ---------------------------------------------------------------------------
// DoctorFinding
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorFinding", frozen)]
pub struct PyDoctorFinding {
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    severity: String,
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    detail: Option<String>,
}

impl From<memvid_core::types::verification::DoctorFinding> for PyDoctorFinding {
    fn from(f: memvid_core::types::verification::DoctorFinding) -> Self {
        Self {
            code: doctor_finding_code_str(&f.code).to_string(),
            severity: doctor_severity_str(&f.severity).to_string(),
            message: f.message,
            detail: f.detail,
        }
    }
}

#[pymethods]
impl PyDoctorFinding {
    fn __repr__(&self) -> String {
        format!(
            "DoctorFinding(code={:?}, severity={:?}, message={:?})",
            self.code, self.severity, self.message
        )
    }
}

// ---------------------------------------------------------------------------
// DoctorActionReport
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorActionReport", frozen)]
pub struct PyDoctorActionReport {
    #[pyo3(get)]
    action: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    detail: Option<String>,
}

impl From<memvid_core::types::verification::DoctorActionReport> for PyDoctorActionReport {
    fn from(a: memvid_core::types::verification::DoctorActionReport) -> Self {
        Self {
            action: doctor_action_kind_str(&a.action).to_string(),
            status: doctor_action_status_str(&a.status).to_string(),
            detail: a.detail,
        }
    }
}

// ---------------------------------------------------------------------------
// DoctorPhaseReport
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorPhaseReport", frozen)]
pub struct PyDoctorPhaseReport {
    #[pyo3(get)]
    phase: String,
    #[pyo3(get)]
    status: String,
    #[pyo3(get)]
    duration_ms: Option<u64>,
    raw_actions: Vec<PyDoctorActionReport>,
}

impl From<memvid_core::types::verification::DoctorPhaseReport> for PyDoctorPhaseReport {
    fn from(p: memvid_core::types::verification::DoctorPhaseReport) -> Self {
        Self {
            phase: doctor_phase_kind_str(&p.phase).to_string(),
            status: doctor_phase_status_str(&p.status).to_string(),
            duration_ms: p.duration_ms,
            raw_actions: p.actions.into_iter().map(PyDoctorActionReport::from).collect(),
        }
    }
}

#[pymethods]
impl PyDoctorPhaseReport {
    #[getter]
    fn actions(&self) -> Vec<PyDoctorActionReport> {
        self.raw_actions
            .iter()
            .map(|a| PyDoctorActionReport {
                action: a.action.clone(),
                status: a.status.clone(),
                detail: a.detail.clone(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DoctorMetrics
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorMetrics", frozen)]
pub struct PyDoctorMetrics {
    #[pyo3(get)]
    total_duration_ms: u64,
    #[pyo3(get)]
    actions_completed: usize,
    #[pyo3(get)]
    actions_skipped: usize,
    raw_phase_durations: Vec<(String, u64)>,
}

impl From<memvid_core::types::verification::DoctorMetrics> for PyDoctorMetrics {
    fn from(m: memvid_core::types::verification::DoctorMetrics) -> Self {
        Self {
            total_duration_ms: m.total_duration_ms,
            actions_completed: m.actions_completed,
            actions_skipped: m.actions_skipped,
            raw_phase_durations: m
                .phase_durations
                .into_iter()
                .map(|pd| (doctor_phase_kind_str(&pd.phase).to_string(), pd.duration_ms))
                .collect(),
        }
    }
}

#[pymethods]
impl PyDoctorMetrics {
    /// Returns list of (phase_name, duration_ms) tuples.
    #[getter]
    fn phase_durations(&self) -> Vec<(String, u64)> {
        self.raw_phase_durations.clone()
    }
}

// ---------------------------------------------------------------------------
// DoctorActionPlan
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[pyclass(name = "DoctorActionPlan", frozen)]
pub struct PyDoctorActionPlan {
    #[pyo3(get)]
    action: String,
    #[pyo3(get)]
    required: bool,
    #[pyo3(get)]
    note: Option<String>,
    raw_reasons: Vec<String>,
}

impl From<memvid_core::types::verification::DoctorActionPlan> for PyDoctorActionPlan {
    fn from(a: memvid_core::types::verification::DoctorActionPlan) -> Self {
        Self {
            action: doctor_action_kind_str(&a.action).to_string(),
            required: a.required,
            note: a.note,
            raw_reasons: a
                .reasons
                .iter()
                .map(|c| doctor_finding_code_str(c).to_string())
                .collect(),
        }
    }
}

#[pymethods]
impl PyDoctorActionPlan {
    #[getter]
    fn reasons(&self) -> Vec<String> {
        self.raw_reasons.clone()
    }
}

// ---------------------------------------------------------------------------
// DoctorPhasePlan
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorPhasePlan", frozen)]
pub struct PyDoctorPhasePlan {
    #[pyo3(get)]
    phase: String,
    raw_actions: Vec<PyDoctorActionPlan>,
}

impl From<memvid_core::types::verification::DoctorPhasePlan> for PyDoctorPhasePlan {
    fn from(p: memvid_core::types::verification::DoctorPhasePlan) -> Self {
        Self {
            phase: doctor_phase_kind_str(&p.phase).to_string(),
            raw_actions: p.actions.into_iter().map(PyDoctorActionPlan::from).collect(),
        }
    }
}

#[pymethods]
impl PyDoctorPhasePlan {
    #[getter]
    fn actions(&self) -> Vec<PyDoctorActionPlan> {
        self.raw_actions
            .iter()
            .map(|a| PyDoctorActionPlan {
                action: a.action.clone(),
                required: a.required,
                note: a.note.clone(),
                raw_reasons: a.raw_reasons.clone(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DoctorPlan
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorPlan", frozen)]
pub struct PyDoctorPlan {
    #[pyo3(get)]
    version: u32,
    #[pyo3(get)]
    file_path: String,
    /// Stash the original Rust plan for doctor_apply round-trip.
    pub(crate) rust_plan: memvid_core::types::DoctorPlan,
    raw_findings: Vec<PyDoctorFinding>,
    raw_phases: Vec<PyDoctorPhasePlan>,
}

impl From<memvid_core::types::DoctorPlan> for PyDoctorPlan {
    fn from(p: memvid_core::types::DoctorPlan) -> Self {
        let file_path = p.file_path.to_string_lossy().into_owned();
        let version = p.version;
        let raw_findings = p.findings.iter().cloned().map(PyDoctorFinding::from).collect();
        let raw_phases = p.phases.iter().cloned().map(PyDoctorPhasePlan::from).collect();
        Self {
            version,
            file_path,
            raw_findings,
            raw_phases,
            rust_plan: p,
        }
    }
}

#[pymethods]
impl PyDoctorPlan {
    #[getter]
    fn findings(&self) -> Vec<PyDoctorFinding> {
        self.raw_findings
            .iter()
            .map(|f| PyDoctorFinding {
                code: f.code.clone(),
                severity: f.severity.clone(),
                message: f.message.clone(),
                detail: f.detail.clone(),
            })
            .collect()
    }

    #[getter]
    fn phases(&self) -> Vec<PyDoctorPhasePlan> {
        self.raw_phases
            .iter()
            .map(|p| PyDoctorPhasePlan {
                phase: p.phase.clone(),
                raw_actions: p.raw_actions.clone(),
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "DoctorPlan(version={}, file_path={:?}, findings={}, phases={})",
            self.version,
            self.file_path,
            self.raw_findings.len(),
            self.raw_phases.len()
        )
    }
}

// ---------------------------------------------------------------------------
// DoctorReport
// ---------------------------------------------------------------------------

#[pyclass(name = "DoctorReport", frozen)]
pub struct PyDoctorReport {
    #[pyo3(get)]
    status: String,
    raw_plan: PyDoctorPlan,
    raw_phases: Vec<PyDoctorPhaseReport>,
    raw_findings: Vec<PyDoctorFinding>,
    raw_metrics: PyDoctorMetrics,
    raw_verification: Option<PyVerificationReport>,
}

impl From<memvid_core::types::DoctorReport> for PyDoctorReport {
    fn from(r: memvid_core::types::DoctorReport) -> Self {
        Self {
            status: doctor_status_str(&r.status).to_string(),
            raw_plan: PyDoctorPlan::from(r.plan),
            raw_phases: r.phases.into_iter().map(PyDoctorPhaseReport::from).collect(),
            raw_findings: r.findings.into_iter().map(PyDoctorFinding::from).collect(),
            raw_metrics: PyDoctorMetrics::from(r.metrics),
            raw_verification: r.verification.map(PyVerificationReport::from),
        }
    }
}

#[pymethods]
impl PyDoctorReport {
    #[getter]
    fn plan(&self) -> PyDoctorPlan {
        PyDoctorPlan {
            version: self.raw_plan.version,
            file_path: self.raw_plan.file_path.clone(),
            rust_plan: self.raw_plan.rust_plan.clone(),
            raw_findings: self
                .raw_plan
                .raw_findings
                .iter()
                .map(|f| PyDoctorFinding {
                    code: f.code.clone(),
                    severity: f.severity.clone(),
                    message: f.message.clone(),
                    detail: f.detail.clone(),
                })
                .collect(),
            raw_phases: self
                .raw_plan
                .raw_phases
                .iter()
                .map(|p| PyDoctorPhasePlan {
                    phase: p.phase.clone(),
                    raw_actions: p.raw_actions.clone(),
                })
                .collect(),
        }
    }

    #[getter]
    fn phases(&self) -> Vec<PyDoctorPhaseReport> {
        self.raw_phases
            .iter()
            .map(|p| PyDoctorPhaseReport {
                phase: p.phase.clone(),
                status: p.status.clone(),
                duration_ms: p.duration_ms,
                raw_actions: p
                    .raw_actions
                    .iter()
                    .map(|a| PyDoctorActionReport {
                        action: a.action.clone(),
                        status: a.status.clone(),
                        detail: a.detail.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    #[getter]
    fn findings(&self) -> Vec<PyDoctorFinding> {
        self.raw_findings
            .iter()
            .map(|f| PyDoctorFinding {
                code: f.code.clone(),
                severity: f.severity.clone(),
                message: f.message.clone(),
                detail: f.detail.clone(),
            })
            .collect()
    }

    #[getter]
    fn metrics(&self) -> PyDoctorMetrics {
        PyDoctorMetrics {
            total_duration_ms: self.raw_metrics.total_duration_ms,
            actions_completed: self.raw_metrics.actions_completed,
            actions_skipped: self.raw_metrics.actions_skipped,
            raw_phase_durations: self.raw_metrics.raw_phase_durations.clone(),
        }
    }

    #[getter]
    fn verification(&self) -> Option<PyVerificationReport> {
        self.raw_verification.as_ref().map(|v| PyVerificationReport {
            file_path: v.file_path.clone(),
            overall_status: v.overall_status.clone(),
            raw_checks: v
                .raw_checks
                .iter()
                .map(|c| PyVerificationCheck {
                    name: c.name.clone(),
                    status: c.status.clone(),
                    details: c.details.clone(),
                })
                .collect(),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "DoctorReport(status={:?}, findings={}, phases={})",
            self.status,
            self.raw_findings.len(),
            self.raw_phases.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[pyclass(name = "Stats", frozen)]
pub struct PyStats {
    #[pyo3(get)]
    frame_count: u64,
    #[pyo3(get)]
    size_bytes: u64,
    #[pyo3(get)]
    tier: String,
    #[pyo3(get)]
    has_lex_index: bool,
    #[pyo3(get)]
    has_vec_index: bool,
    #[pyo3(get)]
    has_clip_index: bool,
    #[pyo3(get)]
    has_time_index: bool,
    #[pyo3(get)]
    seq_no: Option<i64>,
    #[pyo3(get)]
    capacity_bytes: u64,
    #[pyo3(get)]
    active_frame_count: u64,
    #[pyo3(get)]
    payload_bytes: u64,
    #[pyo3(get)]
    logical_bytes: u64,
    #[pyo3(get)]
    saved_bytes: u64,
    #[pyo3(get)]
    compression_ratio_percent: f64,
    #[pyo3(get)]
    savings_percent: f64,
    #[pyo3(get)]
    storage_utilisation_percent: f64,
    #[pyo3(get)]
    remaining_capacity_bytes: u64,
    #[pyo3(get)]
    average_frame_payload_bytes: u64,
    #[pyo3(get)]
    average_frame_logical_bytes: u64,
    #[pyo3(get)]
    wal_bytes: u64,
    #[pyo3(get)]
    lex_index_bytes: u64,
    #[pyo3(get)]
    vec_index_bytes: u64,
    #[pyo3(get)]
    time_index_bytes: u64,
    #[pyo3(get)]
    vector_count: u64,
    #[pyo3(get)]
    clip_image_count: u64,
}

impl From<memvid_core::types::Stats> for PyStats {
    fn from(s: memvid_core::types::Stats) -> Self {
        Self {
            frame_count: s.frame_count,
            size_bytes: s.size_bytes,
            tier: tier_str(&s.tier).to_string(),
            has_lex_index: s.has_lex_index,
            has_vec_index: s.has_vec_index,
            has_clip_index: s.has_clip_index,
            has_time_index: s.has_time_index,
            seq_no: s.seq_no,
            capacity_bytes: s.capacity_bytes,
            active_frame_count: s.active_frame_count,
            payload_bytes: s.payload_bytes,
            logical_bytes: s.logical_bytes,
            saved_bytes: s.saved_bytes,
            compression_ratio_percent: s.compression_ratio_percent,
            savings_percent: s.savings_percent,
            storage_utilisation_percent: s.storage_utilisation_percent,
            remaining_capacity_bytes: s.remaining_capacity_bytes,
            average_frame_payload_bytes: s.average_frame_payload_bytes,
            average_frame_logical_bytes: s.average_frame_logical_bytes,
            wal_bytes: s.wal_bytes,
            lex_index_bytes: s.lex_index_bytes,
            vec_index_bytes: s.vec_index_bytes,
            time_index_bytes: s.time_index_bytes,
            vector_count: s.vector_count,
            clip_image_count: s.clip_image_count,
        }
    }
}

#[pymethods]
impl PyStats {
    fn __repr__(&self) -> String {
        format!(
            "Stats(frame_count={}, size_bytes={}, tier={:?})",
            self.frame_count, self.size_bytes, self.tier
        )
    }
}

// ---------------------------------------------------------------------------
// DoctorOptions helper
// ---------------------------------------------------------------------------

fn build_doctor_options(
    rebuild_time_index: bool,
    rebuild_lex_index: bool,
    rebuild_vec_index: bool,
    vacuum: bool,
    dry_run: bool,
    quiet: bool,
) -> memvid_core::types::DoctorOptions {
    memvid_core::types::DoctorOptions {
        rebuild_time_index,
        rebuild_lex_index,
        rebuild_vec_index,
        vacuum,
        dry_run,
        quiet,
    }
}

// ---------------------------------------------------------------------------
// Methods on PyMemvid
// ---------------------------------------------------------------------------

#[pymethods]
impl PyMemvid {
    // --- static methods (operate on path, no open handle needed) -----------

    /// Verify file integrity. Returns a VerificationReport.
    #[staticmethod]
    #[pyo3(signature = (path, *, deep=false))]
    fn verify(py: Python<'_>, path: &str, deep: bool) -> PyResult<PyVerificationReport> {
        error::catch_panic(py, || {
            let report = memvid_core::Memvid::verify(path, deep)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyVerificationReport::from(report))
        })
    }

    /// Run doctor diagnostics and repair on a file.
    #[staticmethod]
    #[pyo3(signature = (path, *, rebuild_time_index=false, rebuild_lex_index=false, rebuild_vec_index=false, vacuum=false, dry_run=false, quiet=false))]
    fn doctor(
        py: Python<'_>,
        path: &str,
        rebuild_time_index: bool,
        rebuild_lex_index: bool,
        rebuild_vec_index: bool,
        vacuum: bool,
        dry_run: bool,
        quiet: bool,
    ) -> PyResult<PyDoctorReport> {
        error::catch_panic(py, || {
            let options = build_doctor_options(
                rebuild_time_index,
                rebuild_lex_index,
                rebuild_vec_index,
                vacuum,
                dry_run,
                quiet,
            );
            let report = memvid_core::Memvid::doctor(path, options)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyDoctorReport::from(report))
        })
    }

    /// Generate a doctor plan without executing repairs.
    #[staticmethod]
    #[pyo3(signature = (path, *, rebuild_time_index=false, rebuild_lex_index=false, rebuild_vec_index=false, vacuum=false, dry_run=false, quiet=false))]
    fn doctor_plan(
        py: Python<'_>,
        path: &str,
        rebuild_time_index: bool,
        rebuild_lex_index: bool,
        rebuild_vec_index: bool,
        vacuum: bool,
        dry_run: bool,
        quiet: bool,
    ) -> PyResult<PyDoctorPlan> {
        error::catch_panic(py, || {
            let options = build_doctor_options(
                rebuild_time_index,
                rebuild_lex_index,
                rebuild_vec_index,
                vacuum,
                dry_run,
                quiet,
            );
            let plan = memvid_core::Memvid::doctor_plan(path, options)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyDoctorPlan::from(plan))
        })
    }

    /// Apply a previously generated doctor plan.
    #[staticmethod]
    fn doctor_apply(py: Python<'_>, path: &str, plan: &PyDoctorPlan) -> PyResult<PyDoctorReport> {
        error::catch_panic(py, || {
            let rust_plan = plan.rust_plan.clone();
            let report = memvid_core::Memvid::doctor_apply(path, rust_plan)
                .map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyDoctorReport::from(report))
        })
    }

    // --- instance methods --------------------------------------------------

    /// Compact the file by removing deleted frames and reclaiming space.
    fn vacuum(&self, py: Python<'_>) -> PyResult<()> {
        error::catch_panic(py, || {
            let mut lock = guard_memvid!(mut self, py);
            let mv = lock.as_mut().unwrap();
            mv.vacuum().map_err(|e| error::from_memvid_error(py, e))?;
            Ok(())
        })
    }

    /// Return file statistics.
    fn stats(&self, py: Python<'_>) -> PyResult<PyStats> {
        error::catch_panic(py, || {
            let lock = guard_memvid!(self, py);
            let mv = lock.as_ref().unwrap();
            let stats = mv.stats().map_err(|e| error::from_memvid_error(py, e))?;
            Ok(PyStats::from(stats))
        })
    }
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVerificationCheck>()?;
    m.add_class::<PyVerificationReport>()?;
    m.add_class::<PyDoctorFinding>()?;
    m.add_class::<PyDoctorActionReport>()?;
    m.add_class::<PyDoctorPhaseReport>()?;
    m.add_class::<PyDoctorMetrics>()?;
    m.add_class::<PyDoctorActionPlan>()?;
    m.add_class::<PyDoctorPhasePlan>()?;
    m.add_class::<PyDoctorPlan>()?;
    m.add_class::<PyDoctorReport>()?;
    m.add_class::<PyStats>()?;
    Ok(())
}
