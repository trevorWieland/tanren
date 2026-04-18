//! `MethodologyService` — shared CLI/MCP tool service.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::standard::Standard;
use tanren_domain::methodology::task::RequiredGuard;
use tanren_domain::{EventId, SpecId, TaskId};
use tanren_store::Store;
use tanren_store::methodology::{AppendPhaseEventOutboxParams, PhaseEventOutboxEntry};

use super::errors::{MethodologyError, MethodologyResult};
use super::phase_events::PhaseEventAttribution;

const OUTBOX_DRAIN_BATCH_SIZE: u64 = 256;

/// Shared methodology service.
///
/// Both `tanren-cli` and `tanren-mcp` take `Arc<MethodologyService>`.
/// The service is transport-agnostic; transports only supply the
/// caller capability scope and phase name.
#[derive(Debug, Clone)]
pub struct MethodologyService {
    pub(crate) store: Arc<Store>,
    required_guards: Arc<[RequiredGuard]>,
    standards: Arc<[Standard]>,
    phase_events: Option<PhaseEventsRuntime>,
    pub(crate) task_spec_cache: Arc<Mutex<HashMap<TaskId, SpecId>>>,
    pub(crate) signpost_spec_cache: Arc<Mutex<HashMap<tanren_domain::SignpostId, SpecId>>>,
}

/// Runtime context for `phase-events.jsonl` writes.
#[derive(Debug, Clone)]
pub struct PhaseEventsRuntime {
    pub spec_folder: PathBuf,
    pub agent_session_id: String,
}

fn default_required_guards() -> Arc<[RequiredGuard]> {
    Arc::from([
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ])
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
            phase_events: None,
            task_spec_cache: Arc::new(Mutex::new(HashMap::new())),
            signpost_spec_cache: Arc::new(Mutex::new(HashMap::new())),
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
            phase_events: None,
            task_spec_cache: Arc::new(Mutex::new(HashMap::new())),
            signpost_spec_cache: Arc::new(Mutex::new(HashMap::new())),
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
        let mut svc = Self::with_required_guards(store, required_guards);
        svc.phase_events = phase_events;
        if !standards.is_empty() {
            svc.standards = Arc::from(standards.into_boxed_slice());
        }
        svc
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

    /// Runtime context used for `phase-events.jsonl` and enforcement
    /// postflight integration.
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
                    expected: "audited runtime requires a spec-folder context".into(),
                    actual: "missing".into(),
                    remediation:
                        "set --spec-folder / TANREN_SPEC_FOLDER for mutating methodology calls"
                            .into(),
                })?;
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
        let pending = self
            .store
            .load_pending_phase_event_outbox(Some(spec_id), OUTBOX_DRAIN_BATCH_SIZE)
            .await?;
        for row in pending {
            self.process_phase_event_outbox_row(row).await?;
        }
        Ok(())
    }

    async fn process_phase_event_outbox_row(
        &self,
        row: PhaseEventOutboxEntry,
    ) -> MethodologyResult<()> {
        self.store
            .increment_phase_event_outbox_attempt(row.event_id)
            .await?;
        let path = PathBuf::from(&row.spec_folder).join("phase-events.jsonl");
        let already_projected = if row.attempt_count > 0 {
            super::jsonl_contains_event_id(&path, row.event_id)?
        } else {
            false
        };
        if !already_projected
            && let Err(err) = super::append_jsonl_encoded_line(&path, &row.line_json)
        {
            let _ = self
                .store
                .mark_phase_event_outbox_pending_error(row.event_id, &err.to_string())
                .await;
            return Err(err);
        }
        self.store
            .mark_phase_event_outbox_projected(row.event_id)
            .await?;
        Ok(())
    }

    /// Reconcile pending `phase-events.jsonl` outbox rows for one spec folder.
    ///
    /// # Errors
    /// Returns a typed error on query or filesystem failure.
    pub async fn reconcile_phase_events_outbox_for_folder(
        &self,
        spec_folder: &std::path::Path,
    ) -> MethodologyResult<u64> {
        let pending = self
            .store
            .load_pending_phase_event_outbox(None, 10_000)
            .await?;
        let spec_folder = spec_folder.to_string_lossy().to_string();
        let mut projected = 0_u64;
        for row in pending {
            if row.spec_folder != spec_folder {
                continue;
            }
            self.process_phase_event_outbox_row(row).await?;
            projected = projected.saturating_add(1);
        }
        Ok(projected)
    }
}
