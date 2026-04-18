//! `MethodologyService` — shared CLI/MCP tool service.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::Utc;
use tanren_domain::events::{DomainEvent, EventEnvelope};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAbandoned as EvTaskAbandoned, TaskAdherent, TaskAudited,
    TaskCompleted as EvTaskCompleted, TaskCreated as EvTaskCreated, TaskGateChecked,
    TaskImplemented, TaskStarted, TaskXChecked, fold_task_status,
};
use tanren_domain::methodology::standard::Standard;
use tanren_domain::methodology::task::{
    LegalTransition, RequiredGuard, Task, TaskStatus, TaskTransitionKind,
};
use tanren_domain::{EntityRef, EventId, SpecId, TaskId};
use tanren_store::Store;

use tanren_contract::methodology::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, CreateTaskResponse, SchemaVersion,
    StartTaskParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

/// Shared methodology service.
///
/// Both `tanren-cli` and `tanren-mcp` take `Arc<MethodologyService>`.
/// The service is transport-agnostic; transports only supply the
/// caller's [`CapabilityScope`] and phase name.
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
                if self.required_guards.is_empty() {
                    self.emit(
                        phase,
                        MethodologyEvent::TaskCompleted(EvTaskCompleted {
                            task_id: params.task_id,
                            spec_id,
                        }),
                    )
                    .await?;
                }
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
        let events = tanren_store::methodology::projections::load_methodology_events_for_entity(
            self.store(),
            EntityRef::Task(task_id),
            Some(spec_id),
            METHODOLOGY_PAGE_SIZE,
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

    /// `mark_task_guard_satisfied` emits one discrete guard event and,
    /// on convergence, emits `TaskCompleted` in the same call.
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
            guard_event(task_id, spec_id, guard, idempotency_key)?,
        )
        .await?;
        // Re-fold and fire `TaskCompleted` once required guards converge.
        let events = tanren_store::methodology::projections::load_methodology_events_for_entity(
            self.store(),
            EntityRef::Task(task_id),
            Some(spec_id),
            METHODOLOGY_PAGE_SIZE,
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

fn guard_event(
    task_id: TaskId,
    spec_id: SpecId,
    guard: RequiredGuard,
    idempotency_key: Option<String>,
) -> MethodologyResult<MethodologyEvent> {
    let event = match guard {
        RequiredGuard::GateChecked => MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id,
            spec_id,
            idempotency_key,
        }),
        RequiredGuard::Audited => MethodologyEvent::TaskAudited(TaskAudited {
            task_id,
            spec_id,
            idempotency_key,
        }),
        RequiredGuard::Adherent => MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id,
            spec_id,
            idempotency_key,
        }),
        RequiredGuard::Extra(name) => MethodologyEvent::TaskXChecked(TaskXChecked {
            task_id,
            spec_id,
            guard_name: tanren_domain::NonEmptyString::try_new(name).map_err(|e| {
                MethodologyError::FieldValidation {
                    field_path: "/guard".into(),
                    expected: "extra guard name must be non-empty".into(),
                    actual: "empty".into(),
                    remediation: e.to_string(),
                }
            })?,
            idempotency_key,
        }),
    };
    Ok(event)
}
