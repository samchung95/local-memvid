use napi_derive::napi;

use crate::error::from_memvid_error;
use crate::memvid::{guard_memvid, lock_inner, JsMemvid};

// ---------------------------------------------------------------------------
// Helper: enum ↔ string via serde (all Rust enums have rename_all = "snake_case")
// ---------------------------------------------------------------------------

fn enum_to_string<T: serde::Serialize>(val: &T) -> String {
    serde_json::to_value(val)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn string_to_enum<T: serde::de::DeserializeOwned>(s: &str) -> napi::Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(|e| {
        napi::Error::new(
            napi::Status::InvalidArg,
            format!("Invalid enum value '{s}': {e}"),
        )
    })
}

// ---------------------------------------------------------------------------
// JS types — Verification
// ---------------------------------------------------------------------------

#[napi(object)]
pub struct JsVerificationReport {
    pub file_path: String,
    pub checks: Vec<JsVerificationCheck>,
    pub overall_status: String,
}

#[napi(object)]
pub struct JsVerificationCheck {
    pub name: String,
    pub status: String,
    pub details: Option<String>,
}

fn from_verification_check(c: &memvid_core::VerificationCheck) -> JsVerificationCheck {
    JsVerificationCheck {
        name: c.name.clone(),
        status: enum_to_string(&c.status),
        details: c.details.clone(),
    }
}

fn from_verification_report(r: &memvid_core::VerificationReport) -> JsVerificationReport {
    JsVerificationReport {
        file_path: r.file_path.to_string_lossy().into_owned(),
        checks: r.checks.iter().map(from_verification_check).collect(),
        overall_status: enum_to_string(&r.overall_status),
    }
}

// ---------------------------------------------------------------------------
// JS types — Doctor
// ---------------------------------------------------------------------------

#[napi(object)]
pub struct JsDoctorOptions {
    pub rebuild_time_index: Option<bool>,
    pub rebuild_lex_index: Option<bool>,
    pub rebuild_vec_index: Option<bool>,
    pub vacuum: Option<bool>,
    pub dry_run: Option<bool>,
    pub quiet: Option<bool>,
}

fn to_doctor_options(opts: &JsDoctorOptions) -> memvid_core::DoctorOptions {
    memvid_core::DoctorOptions {
        rebuild_time_index: opts.rebuild_time_index.unwrap_or(false),
        rebuild_lex_index: opts.rebuild_lex_index.unwrap_or(false),
        rebuild_vec_index: opts.rebuild_vec_index.unwrap_or(false),
        vacuum: opts.vacuum.unwrap_or(false),
        dry_run: opts.dry_run.unwrap_or(false),
        quiet: opts.quiet.unwrap_or(false),
    }
}

fn from_doctor_options(opts: &memvid_core::DoctorOptions) -> JsDoctorOptions {
    JsDoctorOptions {
        rebuild_time_index: Some(opts.rebuild_time_index),
        rebuild_lex_index: Some(opts.rebuild_lex_index),
        rebuild_vec_index: Some(opts.rebuild_vec_index),
        vacuum: Some(opts.vacuum),
        dry_run: Some(opts.dry_run),
        quiet: Some(opts.quiet),
    }
}

#[napi(object)]
pub struct JsDoctorFinding {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub detail: Option<String>,
}

fn from_doctor_finding(f: &memvid_core::DoctorFinding) -> JsDoctorFinding {
    JsDoctorFinding {
        code: enum_to_string(&f.code),
        severity: enum_to_string(&f.severity),
        message: f.message.clone(),
        detail: f.detail.clone(),
    }
}

fn to_doctor_finding(f: &JsDoctorFinding) -> napi::Result<memvid_core::DoctorFinding> {
    Ok(memvid_core::DoctorFinding {
        code: string_to_enum(&f.code)?,
        severity: string_to_enum(&f.severity)?,
        message: f.message.clone(),
        detail: f.detail.clone(),
    })
}

#[napi(object)]
pub struct JsDoctorActionPlan {
    pub action: String,
    pub required: bool,
    pub reasons: Vec<String>,
    pub note: Option<String>,
    /// JSON-serialized DoctorActionDetail (tagged enum).
    pub detail: Option<String>,
}

fn from_doctor_action_plan(a: &memvid_core::DoctorActionPlan) -> JsDoctorActionPlan {
    JsDoctorActionPlan {
        action: enum_to_string(&a.action),
        required: a.required,
        reasons: a.reasons.iter().map(|r| enum_to_string(r)).collect(),
        note: a.note.clone(),
        detail: a.detail.as_ref().and_then(|d| serde_json::to_string(d).ok()),
    }
}

fn to_doctor_action_plan(a: &JsDoctorActionPlan) -> napi::Result<memvid_core::DoctorActionPlan> {
    let reasons: napi::Result<Vec<_>> = a.reasons.iter().map(|r| string_to_enum(r)).collect();
    Ok(memvid_core::DoctorActionPlan {
        action: string_to_enum(&a.action)?,
        required: a.required,
        reasons: reasons?,
        note: a.note.clone(),
        detail: a
            .detail
            .as_ref()
            .and_then(|d| serde_json::from_str(d).ok()),
    })
}

#[napi(object)]
pub struct JsDoctorPhasePlan {
    pub phase: String,
    pub actions: Vec<JsDoctorActionPlan>,
}

fn from_doctor_phase_plan(p: &memvid_core::DoctorPhasePlan) -> JsDoctorPhasePlan {
    JsDoctorPhasePlan {
        phase: enum_to_string(&p.phase),
        actions: p.actions.iter().map(from_doctor_action_plan).collect(),
    }
}

fn to_doctor_phase_plan(p: &JsDoctorPhasePlan) -> napi::Result<memvid_core::DoctorPhasePlan> {
    let actions: napi::Result<Vec<_>> = p.actions.iter().map(to_doctor_action_plan).collect();
    Ok(memvid_core::DoctorPhasePlan {
        phase: string_to_enum(&p.phase)?,
        actions: actions?,
    })
}

#[napi(object)]
pub struct JsDoctorPlan {
    pub version: u32,
    pub file_path: String,
    pub options: JsDoctorOptions,
    pub findings: Vec<JsDoctorFinding>,
    pub phases: Vec<JsDoctorPhasePlan>,
}

fn from_doctor_plan(p: &memvid_core::DoctorPlan) -> JsDoctorPlan {
    JsDoctorPlan {
        version: p.version,
        file_path: p.file_path.to_string_lossy().into_owned(),
        options: from_doctor_options(&p.options),
        findings: p.findings.iter().map(from_doctor_finding).collect(),
        phases: p.phases.iter().map(from_doctor_phase_plan).collect(),
    }
}

fn to_doctor_plan(p: &JsDoctorPlan) -> napi::Result<memvid_core::DoctorPlan> {
    let findings: napi::Result<Vec<_>> = p.findings.iter().map(to_doctor_finding).collect();
    let phases: napi::Result<Vec<_>> = p.phases.iter().map(to_doctor_phase_plan).collect();
    Ok(memvid_core::DoctorPlan {
        version: p.version,
        file_path: p.file_path.clone().into(),
        options: to_doctor_options(&p.options),
        findings: findings?,
        phases: phases?,
    })
}

#[napi(object)]
pub struct JsDoctorActionReport {
    pub action: String,
    pub status: String,
    pub detail: Option<String>,
}

fn from_doctor_action_report(a: &memvid_core::DoctorActionReport) -> JsDoctorActionReport {
    JsDoctorActionReport {
        action: enum_to_string(&a.action),
        status: enum_to_string(&a.status),
        detail: a.detail.clone(),
    }
}

#[napi(object)]
pub struct JsDoctorPhaseReport {
    pub phase: String,
    pub status: String,
    pub actions: Vec<JsDoctorActionReport>,
    pub duration_ms: Option<f64>,
}

fn from_doctor_phase_report(p: &memvid_core::DoctorPhaseReport) -> JsDoctorPhaseReport {
    JsDoctorPhaseReport {
        phase: enum_to_string(&p.phase),
        status: enum_to_string(&p.status),
        actions: p.actions.iter().map(from_doctor_action_report).collect(),
        duration_ms: p.duration_ms.map(|d| d as f64),
    }
}

#[napi(object)]
pub struct JsDoctorPhaseDuration {
    pub phase: String,
    pub duration_ms: f64,
}

#[napi(object)]
pub struct JsDoctorMetrics {
    pub total_duration_ms: f64,
    pub phase_durations: Vec<JsDoctorPhaseDuration>,
    pub actions_completed: u32,
    pub actions_skipped: u32,
}

fn from_doctor_metrics(m: &memvid_core::DoctorMetrics) -> JsDoctorMetrics {
    JsDoctorMetrics {
        total_duration_ms: m.total_duration_ms as f64,
        phase_durations: m
            .phase_durations
            .iter()
            .map(|d| JsDoctorPhaseDuration {
                phase: enum_to_string(&d.phase),
                duration_ms: d.duration_ms as f64,
            })
            .collect(),
        actions_completed: m.actions_completed as u32,
        actions_skipped: m.actions_skipped as u32,
    }
}

#[napi(object)]
pub struct JsDoctorReport {
    pub plan: JsDoctorPlan,
    pub status: String,
    pub phases: Vec<JsDoctorPhaseReport>,
    pub findings: Vec<JsDoctorFinding>,
    pub metrics: JsDoctorMetrics,
    pub verification: Option<JsVerificationReport>,
}

fn from_doctor_report(r: &memvid_core::DoctorReport) -> JsDoctorReport {
    JsDoctorReport {
        plan: from_doctor_plan(&r.plan),
        status: enum_to_string(&r.status),
        phases: r.phases.iter().map(from_doctor_phase_report).collect(),
        findings: r.findings.iter().map(from_doctor_finding).collect(),
        metrics: from_doctor_metrics(&r.metrics),
        verification: r.verification.as_ref().map(from_verification_report),
    }
}

// ---------------------------------------------------------------------------
// JS types — Stats
// ---------------------------------------------------------------------------

#[napi(object)]
pub struct JsStats {
    pub frame_count: f64,
    pub size_bytes: f64,
    pub tier: String,
    pub has_lex_index: bool,
    pub has_vec_index: bool,
    pub has_clip_index: bool,
    pub has_time_index: bool,
    pub seq_no: Option<i64>,
    pub capacity_bytes: f64,
    pub active_frame_count: f64,
    pub payload_bytes: f64,
    pub logical_bytes: f64,
    pub saved_bytes: f64,
    pub compression_ratio_percent: f64,
    pub savings_percent: f64,
    pub storage_utilisation_percent: f64,
    pub remaining_capacity_bytes: f64,
    pub average_frame_payload_bytes: f64,
    pub average_frame_logical_bytes: f64,
    pub wal_bytes: f64,
    pub lex_index_bytes: f64,
    pub vec_index_bytes: f64,
    pub time_index_bytes: f64,
    pub vector_count: f64,
    pub clip_image_count: f64,
}

fn tier_to_string(tier: &memvid_core::Tier) -> String {
    match tier {
        memvid_core::Tier::Free => "free".to_string(),
        memvid_core::Tier::Dev => "dev".to_string(),
        memvid_core::Tier::Enterprise => "enterprise".to_string(),
    }
}

fn from_stats(s: &memvid_core::Stats) -> JsStats {
    JsStats {
        frame_count: s.frame_count as f64,
        size_bytes: s.size_bytes as f64,
        tier: tier_to_string(&s.tier),
        has_lex_index: s.has_lex_index,
        has_vec_index: s.has_vec_index,
        has_clip_index: s.has_clip_index,
        has_time_index: s.has_time_index,
        seq_no: s.seq_no,
        capacity_bytes: s.capacity_bytes as f64,
        active_frame_count: s.active_frame_count as f64,
        payload_bytes: s.payload_bytes as f64,
        logical_bytes: s.logical_bytes as f64,
        saved_bytes: s.saved_bytes as f64,
        compression_ratio_percent: s.compression_ratio_percent,
        savings_percent: s.savings_percent,
        storage_utilisation_percent: s.storage_utilisation_percent,
        remaining_capacity_bytes: s.remaining_capacity_bytes as f64,
        average_frame_payload_bytes: s.average_frame_payload_bytes as f64,
        average_frame_logical_bytes: s.average_frame_logical_bytes as f64,
        wal_bytes: s.wal_bytes as f64,
        lex_index_bytes: s.lex_index_bytes as f64,
        vec_index_bytes: s.vec_index_bytes as f64,
        time_index_bytes: s.time_index_bytes as f64,
        vector_count: s.vector_count as f64,
        clip_image_count: s.clip_image_count as f64,
    }
}

// ---------------------------------------------------------------------------
// Static methods — Verify
// ---------------------------------------------------------------------------

#[napi]
impl JsMemvid {
    /// Verify file integrity (synchronous).
    #[napi(js_name = "verifySync")]
    pub fn verify_sync(path: String, deep: bool) -> napi::Result<JsVerificationReport> {
        let report = memvid_core::Memvid::verify(&path, deep).map_err(from_memvid_error)?;
        Ok(from_verification_report(&report))
    }

    /// Verify file integrity (async).
    #[napi(js_name = "verify")]
    pub async fn verify_async(path: String, deep: bool) -> napi::Result<JsVerificationReport> {
        let report = tokio::task::spawn_blocking(move || {
            memvid_core::Memvid::verify(&path, deep)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
        .map_err(from_memvid_error)?;
        Ok(from_verification_report(&report))
    }

    // -- Doctor --------------------------------------------------------------

    /// Run diagnostics and auto-heal (synchronous).
    #[napi(js_name = "doctorSync")]
    pub fn doctor_sync(
        path: String,
        options: JsDoctorOptions,
    ) -> napi::Result<JsDoctorReport> {
        let opts = to_doctor_options(&options);
        let report = memvid_core::Memvid::doctor(&path, opts).map_err(from_memvid_error)?;
        Ok(from_doctor_report(&report))
    }

    /// Run diagnostics and auto-heal (async).
    #[napi(js_name = "doctor")]
    pub async fn doctor_async(
        path: String,
        options: JsDoctorOptions,
    ) -> napi::Result<JsDoctorReport> {
        let opts = to_doctor_options(&options);
        let report = tokio::task::spawn_blocking(move || {
            memvid_core::Memvid::doctor(&path, opts)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
        .map_err(from_memvid_error)?;
        Ok(from_doctor_report(&report))
    }

    /// Generate a healing plan without applying it (synchronous).
    #[napi(js_name = "doctorPlanSync")]
    pub fn doctor_plan_sync(
        path: String,
        options: JsDoctorOptions,
    ) -> napi::Result<JsDoctorPlan> {
        let opts = to_doctor_options(&options);
        let plan = memvid_core::Memvid::doctor_plan(&path, opts).map_err(from_memvid_error)?;
        Ok(from_doctor_plan(&plan))
    }

    /// Apply a previously generated healing plan (synchronous).
    #[napi(js_name = "doctorApplySync")]
    pub fn doctor_apply_sync(
        path: String,
        plan: JsDoctorPlan,
    ) -> napi::Result<JsDoctorReport> {
        let rust_plan = to_doctor_plan(&plan)?;
        let report =
            memvid_core::Memvid::doctor_apply(&path, rust_plan).map_err(from_memvid_error)?;
        Ok(from_doctor_report(&report))
    }

    // -- Vacuum --------------------------------------------------------------

    /// Compact the file by removing deleted/superseded frames (synchronous).
    #[napi(js_name = "vacuumSync")]
    pub fn vacuum_sync(&self) -> napi::Result<()> {
        let mut guard = guard_memvid!(self);
        guard.as_mut().unwrap().vacuum().map_err(from_memvid_error)
    }

    /// Compact the file by removing deleted/superseded frames (async).
    #[napi(js_name = "vacuum")]
    pub async fn vacuum_async(&self) -> napi::Result<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = lock_inner(&inner)?;
            guard.as_mut().unwrap().vacuum().map_err(from_memvid_error)
        })
        .await
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("[INTERNAL] {e}")))?
    }

    // -- Stats ---------------------------------------------------------------

    /// Get comprehensive file statistics (synchronous).
    #[napi(js_name = "statsSync")]
    pub fn stats_sync(&self) -> napi::Result<JsStats> {
        let guard = guard_memvid!(self);
        let stats = guard.as_ref().unwrap().stats().map_err(from_memvid_error)?;
        Ok(from_stats(&stats))
    }

    /// Get comprehensive file statistics (async).
    #[napi(js_name = "stats")]
    pub async fn stats_async(&self) -> napi::Result<JsStats> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = lock_inner(&inner)?;
            let stats = guard.as_ref().unwrap().stats().map_err(from_memvid_error)?;
            Ok(from_stats(&stats))
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
    use std::sync::{Arc, Mutex};

    // -- Enum round-trip tests -----------------------------------------------

    #[test]
    fn verification_status_round_trip() {
        use memvid_core::VerificationStatus;
        for status in [
            VerificationStatus::Passed,
            VerificationStatus::Failed,
            VerificationStatus::Skipped,
        ] {
            let s = enum_to_string(&status);
            let back: VerificationStatus = string_to_enum(&s).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn doctor_status_round_trip() {
        use memvid_core::DoctorStatus;
        for status in [
            DoctorStatus::Clean,
            DoctorStatus::Healed,
            DoctorStatus::Partial,
            DoctorStatus::Failed,
            DoctorStatus::PlanOnly,
        ] {
            let s = enum_to_string(&status);
            let back: DoctorStatus = string_to_enum(&s).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn doctor_severity_round_trip() {
        use memvid_core::DoctorSeverity;
        for sev in [
            DoctorSeverity::Info,
            DoctorSeverity::Warning,
            DoctorSeverity::Error,
        ] {
            let s = enum_to_string(&sev);
            let back: DoctorSeverity = string_to_enum(&s).unwrap();
            assert_eq!(sev, back);
        }
    }

    #[test]
    fn doctor_phase_kind_round_trip() {
        use memvid_core::DoctorPhaseKind;
        for kind in [
            DoctorPhaseKind::Probe,
            DoctorPhaseKind::HeaderHealing,
            DoctorPhaseKind::WalReplay,
            DoctorPhaseKind::IndexRebuild,
            DoctorPhaseKind::Vacuum,
            DoctorPhaseKind::Finalize,
            DoctorPhaseKind::Verify,
        ] {
            let s = enum_to_string(&kind);
            let back: DoctorPhaseKind = string_to_enum(&s).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn doctor_action_kind_round_trip() {
        use memvid_core::DoctorActionKind;
        for kind in [
            DoctorActionKind::HealHeaderPointer,
            DoctorActionKind::RebuildTimeIndex,
            DoctorActionKind::VacuumCompaction,
            DoctorActionKind::NoOp,
        ] {
            let s = enum_to_string(&kind);
            let back: DoctorActionKind = string_to_enum(&s).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn doctor_finding_code_round_trip() {
        use memvid_core::DoctorFindingCode;
        for code in [
            DoctorFindingCode::HeaderFooterOffsetMismatch,
            DoctorFindingCode::WalHasPendingRecords,
            DoctorFindingCode::LexIndexMissing,
        ] {
            let s = enum_to_string(&code);
            let back: DoctorFindingCode = string_to_enum(&s).unwrap();
            assert_eq!(code, back);
        }
    }

    // -- Converter tests -----------------------------------------------------

    #[test]
    fn from_verification_report_converts_fields() {
        let report = memvid_core::VerificationReport {
            file_path: "/tmp/test.mv2".into(),
            checks: vec![memvid_core::VerificationCheck {
                name: "header".to_string(),
                status: memvid_core::VerificationStatus::Passed,
                details: Some("ok".to_string()),
            }],
            overall_status: memvid_core::VerificationStatus::Passed,
        };
        let js = from_verification_report(&report);
        assert_eq!(js.file_path, "/tmp/test.mv2");
        assert_eq!(js.checks.len(), 1);
        assert_eq!(js.checks[0].name, "header");
        assert_eq!(js.checks[0].status, "passed");
        assert_eq!(js.checks[0].details.as_deref(), Some("ok"));
        assert_eq!(js.overall_status, "passed");
    }

    #[test]
    fn doctor_options_round_trip() {
        let js_opts = JsDoctorOptions {
            rebuild_time_index: Some(true),
            rebuild_lex_index: None,
            rebuild_vec_index: Some(false),
            vacuum: Some(true),
            dry_run: None,
            quiet: Some(true),
        };
        let rust_opts = to_doctor_options(&js_opts);
        assert!(rust_opts.rebuild_time_index);
        assert!(!rust_opts.rebuild_lex_index); // None → false
        assert!(!rust_opts.rebuild_vec_index);
        assert!(rust_opts.vacuum);
        assert!(!rust_opts.dry_run); // None → false
        assert!(rust_opts.quiet);

        let back = from_doctor_options(&rust_opts);
        assert_eq!(back.rebuild_time_index, Some(true));
        assert_eq!(back.rebuild_lex_index, Some(false));
    }

    #[test]
    fn doctor_finding_round_trip() {
        let finding = memvid_core::DoctorFinding {
            code: memvid_core::DoctorFindingCode::LexIndexMissing,
            severity: memvid_core::DoctorSeverity::Warning,
            message: "Lex index not found".to_string(),
            detail: Some("expected at offset 1024".to_string()),
        };
        let js = from_doctor_finding(&finding);
        assert_eq!(js.code, "lex_index_missing");
        assert_eq!(js.severity, "warning");
        assert_eq!(js.message, "Lex index not found");

        let back = to_doctor_finding(&js).unwrap();
        assert_eq!(back.code, finding.code);
        assert_eq!(back.severity, finding.severity);
        assert_eq!(back.message, finding.message);
        assert_eq!(back.detail, finding.detail);
    }

    #[test]
    fn action_detail_json_round_trip() {
        use memvid_core::DoctorActionDetail;
        let detail = DoctorActionDetail::WalReplay {
            from_sequence: 10,
            to_sequence: 42,
            pending_records: 5,
        };
        let json = serde_json::to_string(&detail).unwrap();
        let back: DoctorActionDetail = serde_json::from_str(&json).unwrap();
        match back {
            DoctorActionDetail::WalReplay {
                from_sequence,
                to_sequence,
                pending_records,
            } => {
                assert_eq!(from_sequence, 10);
                assert_eq!(to_sequence, 42);
                assert_eq!(pending_records, 5);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn tier_to_string_maps_correctly() {
        assert_eq!(tier_to_string(&memvid_core::Tier::Free), "free");
        assert_eq!(tier_to_string(&memvid_core::Tier::Dev), "dev");
        assert_eq!(tier_to_string(&memvid_core::Tier::Enterprise), "enterprise");
    }

    #[test]
    fn stats_converter_maps_fields() {
        let stats = memvid_core::Stats {
            frame_count: 100,
            size_bytes: 4096,
            tier: memvid_core::Tier::Dev,
            has_lex_index: true,
            has_vec_index: false,
            has_clip_index: false,
            has_time_index: true,
            seq_no: Some(42),
            capacity_bytes: 1_000_000,
            active_frame_count: 95,
            payload_bytes: 3000,
            logical_bytes: 3500,
            saved_bytes: 500,
            compression_ratio_percent: 85.7,
            savings_percent: 14.3,
            storage_utilisation_percent: 0.4,
            remaining_capacity_bytes: 996_000,
            average_frame_payload_bytes: 30,
            average_frame_logical_bytes: 35,
            wal_bytes: 128,
            lex_index_bytes: 512,
            vec_index_bytes: 0,
            time_index_bytes: 256,
            vector_count: 0,
            clip_image_count: 0,
        };
        let js = from_stats(&stats);
        assert_eq!(js.frame_count, 100.0);
        assert_eq!(js.size_bytes, 4096.0);
        assert_eq!(js.tier, "dev");
        assert!(js.has_lex_index);
        assert!(!js.has_vec_index);
        assert!(js.has_time_index);
        assert_eq!(js.seq_no, Some(42));
        assert_eq!(js.active_frame_count, 95.0);
        assert_eq!(js.compression_ratio_percent, 85.7);
    }

    // -- Guard tests ---------------------------------------------------------

    #[test]
    fn vacuum_rejects_closed_instance() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.vacuum_sync();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("[CLOSED]"));
    }

    #[test]
    fn stats_rejects_closed_instance() {
        let js = JsMemvid {
            inner: Arc::new(Mutex::new(None)),
        };
        let result = js.stats_sync();
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.to_string().contains("[CLOSED]"));
    }
}
