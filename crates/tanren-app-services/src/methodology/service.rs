//! `MethodologyService` — shared CLI/MCP tool service.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::pillar::{Pillar, builtin_pillars};
use tanren_domain::methodology::standard::Standard;
use tanren_domain::methodology::task::RequiredGuard;
use tanren_domain::{EventId, SpecId};
use tanren_store::Store;
use tanren_store::methodology::{
    AppendPhaseEventOutboxParams, PhaseEventOutboxCursor, PhaseEventOutboxEntry,
};
use tanren_store::{
    ConsumeActorTokenJtiParams, PurgeExpiredActorTokenJtisParams, TokenReplayStore,
};

use super::errors::{MethodologyError, MethodologyResult};
use super::phase_events::{
    PhaseEventAttribution, PhaseEventsAppendPolicy, PhaseEventsCompactionReport,
};

const OUTBOX_DRAIN_BATCH_SIZE: u64 = 256;
const OUTBOX_DRAIN_ROW_BUDGET: u64 = OUTBOX_DRAIN_BATCH_SIZE * 8;
const OUTBOX_DRAIN_TIME_BUDGET_MS: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodologyRuntimeTuning {
    pub phase_events_append_policy: PhaseEventsAppendPolicy,
    pub phase_events_compaction_min_lines: usize,
    pub projection_checkpoint_compaction_append_threshold: usize,
}

impl Default for MethodologyRuntimeTuning {
    fn default() -> Self {
        Self {
            phase_events_append_policy: PhaseEventsAppendPolicy::default(),
            phase_events_compaction_min_lines: 10_000,
            projection_checkpoint_compaction_append_threshold: 200,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MethodologyService {
    pub(crate) store: Arc<Store>,
    required_guards: Arc<[RequiredGuard]>,
    standards: Arc<[Standard]>,
    pillars: Arc<[Pillar]>,
    issue_provider: Arc<str>,
    phase_events: Option<PhaseEventsRuntime>,
    runtime_tuning: MethodologyRuntimeTuning,
}

#[derive(Debug, Clone)]
pub struct PhaseEventsRuntime {
    pub spec_id: SpecId,
    pub spec_folder: PathBuf,
    pub agent_session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectionReconcileReport {
    pub tasks_rebuilt: u64,
    pub task_spec_rows_repaired: u64,
    pub signpost_spec_rows_repaired: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PhaseEventsMaintenanceReport {
    pub total_lines_before: u64,
    pub total_lines_after: u64,
    pub duplicates_removed: u64,
    pub empty_lines_removed: u64,
    pub rewrote_file: bool,
}

fn default_required_guards() -> Arc<[RequiredGuard]> {
    Arc::from([
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ])
}

fn default_issue_provider() -> Arc<str> {
    Arc::from("GitHub")
}

impl MethodologyService {
    /// Construct a service over a shared store handle using the default
    /// `task_complete_requires = [gate_checked, audited, adherent]` set.
    #[must_use]
    pub fn new(store: Arc<Store>) -> Self {
        Self {
            store,
            required_guards: default_required_guards(),
            standards: Arc::from(super::standards::baseline_standards().into_boxed_slice()),
            pillars: Arc::from(builtin_pillars().into_boxed_slice()),
            issue_provider: default_issue_provider(),
            phase_events: None,
            runtime_tuning: MethodologyRuntimeTuning::default(),
        }
    }

    /// Construct a service with a config-driven required-guard set.
    ///
    /// The set is what `fold_task_status` will check before emitting a
    /// `TaskCompleted` event. Passing an empty slice collapses to
    /// "complete on `TaskImplemented`"; duplicates are silently
    /// de-duplicated by value.
    #[must_use]
    pub fn with_required_guards(store: Arc<Store>, required_guards: Vec<RequiredGuard>) -> Self {
        let mut seen: Vec<RequiredGuard> = Vec::with_capacity(required_guards.len());
        for g in required_guards {
            if !seen.contains(&g) {
                seen.push(g);
            }
        }
        Self {
            store,
            required_guards: Arc::from(seen.into_boxed_slice()),
            standards: Arc::from(super::standards::baseline_standards().into_boxed_slice()),
            pillars: Arc::from(builtin_pillars().into_boxed_slice()),
            issue_provider: default_issue_provider(),
            phase_events: None,
            runtime_tuning: MethodologyRuntimeTuning::default(),
        }
    }

    /// Construct a service with required guards and phase-event runtime.
    #[must_use]
    pub fn with_runtime(
        store: Arc<Store>,
        required_guards: Vec<RequiredGuard>,
        phase_events: Option<PhaseEventsRuntime>,
        standards: Vec<Standard>,
    ) -> Self {
        Self::with_runtime_and_pillars(store, required_guards, phase_events, standards, vec![])
    }

    /// Construct a service with required guards, phase-event runtime, standards,
    /// and an explicit rubric pillar registry.
    #[must_use]
    pub fn with_runtime_and_pillars(
        store: Arc<Store>,
        required_guards: Vec<RequiredGuard>,
        phase_events: Option<PhaseEventsRuntime>,
        standards: Vec<Standard>,
        pillars: Vec<Pillar>,
    ) -> Self {
        Self::with_runtime_and_pillars_and_issue_provider(
            store,
            required_guards,
            phase_events,
            standards,
            pillars,
            "GitHub",
        )
    }

    /// Construct a service with required guards, phase-event runtime, standards,
    /// an explicit rubric pillar registry, and a configured issue provider.
    #[must_use]
    pub fn with_runtime_and_pillars_and_issue_provider(
        store: Arc<Store>,
        required_guards: Vec<RequiredGuard>,
        phase_events: Option<PhaseEventsRuntime>,
        standards: Vec<Standard>,
        pillars: Vec<Pillar>,
        issue_provider: &str,
    ) -> Self {
        let mut svc = Self::with_required_guards(store, required_guards);
        svc.phase_events = phase_events;
        if !standards.is_empty() {
            svc.standards = Arc::from(standards.into_boxed_slice());
        }
        if !pillars.is_empty() {
            svc.pillars = Arc::from(pillars.into_boxed_slice());
        }
        let trimmed = issue_provider.trim();
        if !trimmed.is_empty() {
            svc.issue_provider = Arc::from(trimmed.to_owned());
        }
        svc
    }

    pub fn set_runtime_tuning(&mut self, tuning: MethodologyRuntimeTuning) {
        self.runtime_tuning = tuning;
    }

    #[must_use]
    pub(crate) fn runtime_tuning(&self) -> MethodologyRuntimeTuning {
        self.runtime_tuning
    }

    /// Read the configured required-guard set. Used by projections and
    /// tests to assert config-driven behavior.
    #[must_use]
    pub fn required_guards(&self) -> &[RequiredGuard] {
        &self.required_guards
    }

    /// Runtime standards registry used by adherence + relevance queries.
    #[must_use]
    pub fn standards(&self) -> &[Standard] {
        &self.standards
    }

    /// Runtime rubric pillar registry used by rubric-scoring tools.
    #[must_use]
    pub fn pillars(&self) -> &[Pillar] {
        &self.pillars
    }

    /// Runtime issue-provider tag from config variables.
    #[must_use]
    pub fn issue_provider(&self) -> &str {
        &self.issue_provider
    }

    #[must_use]
    pub fn phase_events_runtime(&self) -> Option<PhaseEventsRuntime> {
        self.phase_events.clone()
    }

    fn new_envelope(payload: MethodologyEvent) -> EventEnvelope {
        EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::new(),
            timestamp: Utc::now(),
            entity_ref: payload.entity_root(),
            payload: DomainEvent::Methodology { event: payload },
        }
    }

    /// Consume one replay guard key for the MCP signed-envelope boundary.
    ///
    /// Returns `Ok(true)` when the `(issuer, audience, jti)` tuple is
    /// newly consumed, and `Ok(false)` when it has already been used.
    ///
    /// # Errors
    /// Returns a store/database error on persistence failures.
    pub async fn consume_replay_guard_once(
        &self,
        issuer: String,
        audience: String,
        jti: String,
        iat_unix: i64,
        exp_unix: i64,
    ) -> MethodologyResult<bool> {
        self.store
            .consume_actor_token_jti(ConsumeActorTokenJtiParams {
                issuer,
                audience,
                jti,
                iat_unix,
                exp_unix,
                consumed_at: Utc::now(),
            })
            .await
            .map_err(MethodologyError::from)
    }

    /// Best-effort bounded purge of expired replay-guard rows.
    ///
    /// # Errors
    /// Returns a store/database error on purge failures.
    pub async fn purge_expired_replay_guards(
        &self,
        expires_before_unix: i64,
        limit: u64,
    ) -> MethodologyResult<u64> {
        self.store
            .purge_expired_actor_token_jtis(PurgeExpiredActorTokenJtisParams {
                expires_before_unix,
                limit,
            })
            .await
            .map_err(MethodologyError::from)
    }

    pub(crate) async fn emit(
        &self,
        phase: &PhaseId,
        event: MethodologyEvent,
    ) -> MethodologyResult<EventEnvelope> {
        self.emit_with_attribution(phase, event, PhaseEventAttribution::default())
            .await
    }

    pub(crate) async fn emit_with_attribution(
        &self,
        phase: &PhaseId,
        event: MethodologyEvent,
        attribution: PhaseEventAttribution,
    ) -> MethodologyResult<EventEnvelope> {
        let envelope = Self::new_envelope(event);
        let outbox = self.phase_event_outbox(phase, &envelope, &attribution)?;
        let drain_spec = outbox.as_ref().map(|(spec_id, _)| *spec_id);
        let outbox_payload = outbox.map(|(_, payload)| payload);
        self.store
            .append_methodology_event_with_outbox(&envelope, outbox_payload)
            .await?;
        if let Some(spec_id) = drain_spec {
            self.drain_phase_event_outbox_for_spec(spec_id).await?;
        }
        Ok(envelope)
    }

    fn phase_event_outbox(
        &self,
        phase: &PhaseId,
        envelope: &EventEnvelope,
        attribution: &PhaseEventAttribution,
    ) -> MethodologyResult<Option<(SpecId, AppendPhaseEventOutboxParams)>> {
        let DomainEvent::Methodology { event } = &envelope.payload else {
            return Ok(None);
        };
        let Some(spec_id) = event.spec_id() else {
            return Ok(None);
        };
        let runtime =
            self.phase_events
                .as_ref()
                .ok_or_else(|| MethodologyError::FieldValidation {
                    field_path: "/spec_folder".into(),
                    expected: "audited runtime requires spec_id + spec_folder context".into(),
                    actual: "missing".into(),
                    remediation:
                        "set --spec-id/--spec-folder or TANREN_SPEC_ID/TANREN_SPEC_FOLDER for mutating methodology calls".into(),
                })?;
        if runtime.spec_id != spec_id {
            return Err(MethodologyError::FieldValidation {
                field_path: "/spec_id".into(),
                expected: runtime.spec_id.to_string(),
                actual: spec_id.to_string(),
                remediation:
                    "ensure every mutation tool call targets the canonical session spec_id".into(),
            });
        }
        let line = super::line_for_envelope_with_attribution(
            envelope,
            spec_id,
            phase.as_str(),
            &runtime.agent_session_id,
            attribution,
        );
        let Some(line) = line else {
            return Ok(None);
        };
        let line_json =
            serde_json::to_string(&line).map_err(|e| MethodologyError::Internal(e.to_string()))?;
        Ok(Some((
            spec_id,
            AppendPhaseEventOutboxParams {
                spec_id,
                spec_folder: runtime.spec_folder.to_string_lossy().to_string(),
                line_json,
            },
        )))
    }

    async fn drain_phase_event_outbox_for_spec(&self, spec_id: SpecId) -> MethodologyResult<()> {
        let started = std::time::Instant::now();
        let time_budget = std::time::Duration::from_millis(OUTBOX_DRAIN_TIME_BUDGET_MS);
        let mut remaining_rows = OUTBOX_DRAIN_ROW_BUDGET;
        let mut cursor: Option<PhaseEventOutboxCursor> = None;
        while remaining_rows > 0 && started.elapsed() < time_budget {
            let pending = self
                .store
                .load_pending_phase_event_outbox_with_cursor(
                    Some(spec_id),
                    cursor,
                    OUTBOX_DRAIN_BATCH_SIZE.min(remaining_rows),
                )
                .await?;
            if pending.is_empty() {
                return Ok(());
            }
            for row in pending {
                cursor = Some(cursor_for_row(&row));
                let _ = self.process_phase_event_outbox_row(row).await?;
                remaining_rows = remaining_rows.saturating_sub(1);
                if remaining_rows == 0 || started.elapsed() >= time_budget {
                    break;
                }
            }
        }
        Ok(())
    }

    async fn process_phase_event_outbox_row(
        &self,
        row: PhaseEventOutboxEntry,
    ) -> MethodologyResult<bool> {
        let claimed = self
            .store
            .claim_phase_event_outbox_pending(row.event_id, row.attempt_count)
            .await?;
        if !claimed {
            return Ok(false);
        }
        let path = PathBuf::from(&row.spec_folder).join("phase-events.jsonl");
        if let Err(err) = super::append_jsonl_encoded_line_if_missing_event_id_with_policy(
            &path,
            &row.line_json,
            Some(row.event_id),
            self.runtime_tuning.phase_events_append_policy,
        ) {
            let _ = self
                .store
                .mark_phase_event_outbox_pending_error(row.event_id, &err.to_string())
                .await;
            return Err(err);
        }
        let projected = self
            .store
            .mark_phase_event_outbox_projected(row.event_id)
            .await?;
        Ok(projected)
    }

    pub async fn reconcile_phase_events_outbox_for_folder(
        &self,
        spec_folder: &std::path::Path,
    ) -> MethodologyResult<u64> {
        let spec_folder_path = spec_folder.to_path_buf();
        let spec_folder = spec_folder.to_string_lossy().to_string();
        let runtime_spec_filter = self.phase_events.as_ref().and_then(|runtime| {
            (runtime.spec_folder.to_string_lossy() == spec_folder).then_some(runtime.spec_id)
        });
        let mut cursor: Option<PhaseEventOutboxCursor> = None;
        let mut projected = 0_u64;
        loop {
            let pending = self
                .store
                .load_pending_phase_event_outbox_for_folder(
                    &spec_folder,
                    runtime_spec_filter,
                    cursor,
                    OUTBOX_DRAIN_BATCH_SIZE,
                )
                .await?;
            if pending.is_empty() {
                break;
            }
            for row in pending {
                cursor = Some(cursor_for_row(&row));
                if self.process_phase_event_outbox_row(row).await? {
                    projected = projected.saturating_add(1);
                }
            }
        }
        if let Some(spec_id) =
            runtime_spec_filter.or_else(|| infer_spec_id_from_phase_events(&spec_folder_path))
        {
            self.materialize_projected_artifacts(spec_id, &spec_folder_path)?;
        }
        Ok(projected)
    }

    pub fn compact_phase_events_for_folder(
        &self,
        spec_folder: &std::path::Path,
    ) -> MethodologyResult<PhaseEventsMaintenanceReport> {
        let path = spec_folder.join("phase-events.jsonl");
        let report: PhaseEventsCompactionReport = super::compact_jsonl_event_log(&path)?;
        Ok(PhaseEventsMaintenanceReport {
            total_lines_before: report.total_lines_before,
            total_lines_after: report.total_lines_after,
            duplicates_removed: report.duplicates_removed,
            empty_lines_removed: report.empty_lines_removed,
            rewrote_file: report.rewrote_file,
        })
    }
}

fn cursor_for_row(row: &PhaseEventOutboxEntry) -> PhaseEventOutboxCursor {
    PhaseEventOutboxCursor {
        created_at: row.created_at,
        event_id: row.event_id,
    }
}

fn infer_spec_id_from_phase_events(spec_folder: &std::path::Path) -> Option<SpecId> {
    let path = spec_folder.join("phase-events.jsonl");
    let raw = std::fs::read_to_string(path).ok()?;
    raw.lines()
        .find(|line| !line.trim().is_empty())
        .and_then(|line| serde_json::from_str::<super::phase_events::PhaseEventLine>(line).ok())
        .map(|line| line.spec_id)
}
