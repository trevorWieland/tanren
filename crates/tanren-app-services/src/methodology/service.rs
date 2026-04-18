//! `MethodologyService` — the concrete service both CLI and MCP
//! transports call. One method per tool in the catalog. Each method:
//!
//! 1. Enforces the caller's capability scope via
//!    [`super::capabilities::enforce`].
//! 2. Validates inputs against the domain invariants.
//! 3. Emits exactly one `DomainEvent::Methodology { .. }` via
//!    `tanren_store::Store::append_methodology_event`.
//! 4. Returns the typed response per the contract.
//!
//! Tool methods are small (≤ 100 lines) and uniform so tool-catalog
//! growth stays boilerplate-minimal.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAbandoned as EvTaskAbandoned, TaskCompleted as EvTaskCompleted,
    TaskCreated as EvTaskCreated, TaskGuardSatisfied, TaskImplemented, TaskStarted,
    fold_task_status,
};
use tanren_domain::methodology::task::{
    LegalTransition, RequiredGuard, Task, TaskStatus, TaskTransitionKind,
};
use tanren_domain::{EventId, SpecId, TaskId};
use tanren_store::Store;

use tanren_contract::methodology::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, CreateTaskResponse, SchemaVersion,
    StartTaskParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};

/// Shared methodology service.
///
/// Both `tanren-cli` and `tanren-mcp` take `Arc<MethodologyService>`.
/// The service is transport-agnostic; transports only supply the
/// caller's [`CapabilityScope`] and phase name.
#[derive(Debug, Clone)]
pub struct MethodologyService {
    pub(crate) store: Arc<Store>,
    required_guards: Arc<[RequiredGuard]>,
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
    ) -> Self {
        let mut svc = Self::with_required_guards(store, required_guards);
        svc.phase_events = phase_events;
        svc
    }

    /// Read the configured required-guard set. Used by projections and
    /// tests to assert config-driven behavior.
    #[must_use]
    pub fn required_guards(&self) -> &[RequiredGuard] {
        &self.required_guards
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
        phase: &str,
        event: MethodologyEvent,
    ) -> MethodologyResult<EventEnvelope> {
        let envelope = Self::new_envelope(event);
        self.store.append_methodology_event(&envelope).await?;
        self.append_phase_event_line(phase, &envelope)?;
        Ok(envelope)
    }

    fn append_phase_event_line(
        &self,
        phase: &str,
        envelope: &EventEnvelope,
    ) -> MethodologyResult<()> {
        let Some(runtime) = &self.phase_events else {
            return Ok(());
        };
        let DomainEvent::Methodology { event } = &envelope.payload else {
            return Ok(());
        };
        let Some(spec_id) = event.spec_id() else {
            return Ok(());
        };
        let line = super::line_for_envelope(envelope, spec_id, phase, &runtime.agent_session_id);
        let Some(line) = line else {
            return Ok(());
        };
        let path = runtime.spec_folder.join("phase-events.jsonl");
        super::append_jsonl_line_atomic(&path, &line)?;
        Ok(())
    }

    // -- §3.1 Core task operations -------------------------------------------

    /// `create_task` — emit [`MethodologyEvent::TaskCreated`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn create_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: CreateTaskParams,
    ) -> MethodologyResult<CreateTaskResponse> {
        enforce(scope, ToolCapability::TaskCreate, phase)?;
        let title = require_non_empty("/title", &params.title, Some(160))?;
        if let Some(key) = params.idempotency_key.as_deref()
            && let Some(existing) = self
                .find_task_created_by_idempotency(params.spec_id, key)
                .await?
        {
            let semantically_same = existing.title == title
                && existing.description == params.description
                && existing.origin == params.origin
                && existing.acceptance_criteria == params.acceptance_criteria
                && existing.depends_on == params.depends_on
                && existing.parent_task_id == params.parent_task_id;
            if !semantically_same {
                return Err(MethodologyError::Conflict {
                    resource: "create_task".into(),
                    reason: format!(
                        "idempotency_key `{key}` already used with different payload for task {}",
                        existing.id
                    ),
                });
            }
            return Ok(CreateTaskResponse {
                schema_version: SchemaVersion::current(),
                task_id: existing.id,
            });
        }
        let now = Utc::now();
        let task = Task {
            id: TaskId::new(),
            spec_id: params.spec_id,
            title,
            description: params.description,
            acceptance_criteria: params.acceptance_criteria,
            origin: params.origin.clone(),
            status: TaskStatus::Pending,
            depends_on: params.depends_on,
            parent_task_id: params.parent_task_id,
            created_at: now,
            updated_at: now,
        };
        let task_id = task.id;
        self.emit(
            phase,
            MethodologyEvent::TaskCreated(EvTaskCreated {
                task: Box::new(task),
                origin: params.origin,
                idempotency_key: params.idempotency_key,
            }),
        )
        .await?;
        if let Ok(mut cache) = self.task_spec_cache.lock() {
            cache.insert(task_id, params.spec_id);
        }
        Ok(CreateTaskResponse {
            schema_version: SchemaVersion::current(),
            task_id,
        })
    }

    /// `start_task` — emit [`MethodologyEvent::TaskStarted`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn start_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: StartTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskStart, phase)?;
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        match self
            .check_transition(spec_id, params.task_id, TaskTransitionKind::Start)
            .await?
        {
            LegalTransition::Transition => {
                self.emit(
                    phase,
                    MethodologyEvent::TaskStarted(TaskStarted {
                        task_id: params.task_id,
                        spec_id,
                    }),
                )
                .await?;
            }
            LegalTransition::Idempotent => {}
        }
        Ok(())
    }

    /// `complete_task` — emit [`MethodologyEvent::TaskImplemented`].
    /// (The `TaskCompleted` transition to terminal state fires later,
    /// once all required guards have arrived.)
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn complete_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: CompleteTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskComplete, phase)?;
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        match self
            .check_transition(spec_id, params.task_id, TaskTransitionKind::Implement)
            .await?
        {
            LegalTransition::Transition => {
                self.emit(
                    phase,
                    MethodologyEvent::TaskImplemented(TaskImplemented {
                        task_id: params.task_id,
                        spec_id,
                        evidence_refs: params.evidence_refs,
                    }),
                )
                .await?;
            }
            LegalTransition::Idempotent => {}
        }
        Ok(())
    }

    /// `abandon_task` — emit [`MethodologyEvent::TaskAbandoned`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn abandon_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: AbandonTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskAbandon, phase)?;
        // F8: abandon requires either a non-empty `reason` *and* at
        // least one replacement task, or an explicit user-discard note.
        // The contract accepts both as a single shape; replacements
        // non-empty implies "replaced" semantics, empty implies
        // "user-discarded" and the reason must be substantive.
        let reason = require_non_empty("/reason", &params.reason, Some(500))?;
        if params.replacements.is_empty() && reason.as_str().trim().len() < 8 {
            return Err(MethodologyError::FieldValidation {
                field_path: "/replacements".into(),
                expected: "non-empty replacements[] OR /reason describing the discard"
                    .into(),
                actual: format!(
                    "replacements=[], reason={} chars",
                    reason.as_str().trim().len()
                ),
                remediation:
                    "either supply at least one replacement task id, or describe the discard in /reason (≥ 8 chars)".into(),
            });
        }
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        // Legal from any non-terminal state; idempotent if already
        // abandoned with the same replacements (content hash elsewhere).
        match self
            .check_transition(spec_id, params.task_id, TaskTransitionKind::Abandon)
            .await?
        {
            LegalTransition::Transition => {
                self.emit(
                    phase,
                    MethodologyEvent::TaskAbandoned(EvTaskAbandoned {
                        task_id: params.task_id,
                        spec_id,
                        reason,
                        replacements: params.replacements,
                    }),
                )
                .await?;
            }
            LegalTransition::Idempotent => {}
        }
        Ok(())
    }

    /// Fold the current status of `task_id` from the event log and
    /// consult [`TaskStatus::legal_next`]. Returns a typed
    /// [`MethodologyError::IllegalTaskTransition`] on rejection.
    async fn check_transition(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
        kind: TaskTransitionKind,
    ) -> MethodologyResult<LegalTransition> {
        let events = tanren_store::methodology::projections::load_methodology_events(
            self.store(),
            spec_id,
            100_000u64,
        )
        .await?;
        let current = fold_task_status(task_id, &self.required_guards, events.iter())
            .unwrap_or(TaskStatus::Pending);
        current
            .legal_next(kind)
            .map_err(|e| MethodologyError::IllegalTaskTransition {
                task_id,
                from: e.from.to_owned(),
                attempted: e.attempted.to_owned(),
            })
    }

    /// `mark_task_guard_satisfied` — emit one
    /// [`MethodologyEvent::TaskGuardSatisfied`] and, if the accumulated
    /// guard set now satisfies the configured `required_guards`, also
    /// emit a [`MethodologyEvent::TaskCompleted`] in the same call.
    ///
    /// `idempotency_key` is an optional content-addressed key derived
    /// by the caller (tool dispatch layer) from
    /// `(tool_name, payload_canonical_json, task_id)`. When supplied,
    /// repeating the same call is a no-op because `fold_task_status`
    /// treats two `TaskGuardSatisfied` events with the same
    /// `(task_id, guard)` as idempotent by construction — the second
    /// append still lands in the event log but the resulting projection
    /// is identical. Store-level dedup (see
    /// `tanren-store::methodology::append`) rejects duplicate keys
    /// before the event is written.
    ///
    /// # Errors
    /// See [`MethodologyError`]. Propagates
    /// [`MethodologyError::IllegalTaskTransition`] if the task is in a
    /// state that cannot accept a guard event.
    pub async fn mark_task_guard_satisfied(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        task_id: TaskId,
        guard: RequiredGuard,
        idempotency_key: Option<String>,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskComplete, phase)?;
        let spec_id = self.resolve_spec_for_task(task_id).await?;
        match self
            .check_transition(spec_id, task_id, TaskTransitionKind::Guard)
            .await?
        {
            LegalTransition::Idempotent => return Ok(()),
            LegalTransition::Transition => {}
        }
        self.emit(
            phase,
            MethodologyEvent::TaskGuardSatisfied(TaskGuardSatisfied {
                task_id,
                spec_id,
                guard,
                idempotency_key,
            }),
        )
        .await?;
        // Re-fold and, if the accumulated guard set now satisfies
        // `required_guards`, deterministically fire `TaskCompleted` in
        // the same transaction-bundle. This closes the "tasks can stall
        // in implemented state" hole flagged in the audit.
        let events = tanren_store::methodology::projections::load_methodology_events(
            self.store(),
            spec_id,
            100_000u64,
        )
        .await?;
        let status = fold_task_status(task_id, &self.required_guards, events.iter())
            .unwrap_or(TaskStatus::Pending);
        if let TaskStatus::Implemented { guards } = status
            && guards.satisfies(&self.required_guards)
        {
            self.emit(
                phase,
                MethodologyEvent::TaskCompleted(EvTaskCompleted { task_id, spec_id }),
            )
            .await?;
        }
        Ok(())
    }
}
