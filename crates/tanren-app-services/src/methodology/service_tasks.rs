//! Core task lifecycle methods for [`MethodologyService`].

use chrono::Utc;
use serde::Serialize;
use tanren_contract::methodology::{
    AbandonTaskParams, CompleteTaskParams, CreateTaskParams, CreateTaskResponse, SchemaVersion,
    StartTaskParams,
};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAbandoned as EvTaskAbandoned, TaskAdherent, TaskAudited,
    TaskCompleted as EvTaskCompleted, TaskCreated as EvTaskCreated, TaskGateChecked,
    TaskImplemented, TaskStarted, TaskXChecked, fold_task_status,
};
use tanren_domain::methodology::task::{
    LegalTransition, RequiredGuard, Task, TaskStatus, TaskTransitionKind,
};
use tanren_domain::{EntityRef, SpecId, TaskId};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};
use super::service::MethodologyService;

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

impl MethodologyService {
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
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "create_task",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let title = require_non_empty("/title", &params.title, Some(160))?;
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
            },
        )
        .await
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
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "start_task",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
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
            },
        )
        .await
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
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "complete_task",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
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
                        if self.required_guards().is_empty() {
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
            },
        )
        .await
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
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "abandon_task",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
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
            },
        )
        .await
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
        let current = fold_task_status(task_id, self.required_guards(), events.iter())
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
        let payload = GuardMutationPayload {
            task_id,
            guard: guard.clone(),
            idempotency_key: idempotency_key.clone(),
        };
        self.run_idempotent_mutation(
            "mark_task_guard_satisfied",
            spec_id,
            idempotency_key.clone(),
            &payload,
            || async {
                match self
                    .check_transition(spec_id, task_id, TaskTransitionKind::Guard)
                    .await?
                {
                    LegalTransition::Idempotent => return Ok(()),
                    LegalTransition::Transition => {}
                }
                self.emit(
                    phase,
                    guard_event(task_id, spec_id, guard.clone(), idempotency_key.clone())?,
                )
                .await?;
                // Re-fold and fire `TaskCompleted` once required guards converge.
                let events =
                    tanren_store::methodology::projections::load_methodology_events_for_entity(
                        self.store(),
                        EntityRef::Task(task_id),
                        Some(spec_id),
                        METHODOLOGY_PAGE_SIZE,
                    )
                    .await?;
                let status = fold_task_status(task_id, self.required_guards(), events.iter())
                    .unwrap_or(TaskStatus::Pending);
                if let TaskStatus::Implemented { guards } = status
                    && guards.satisfies(self.required_guards())
                {
                    self.emit(
                        phase,
                        MethodologyEvent::TaskCompleted(EvTaskCompleted { task_id, spec_id }),
                    )
                    .await?;
                }
                Ok(())
            },
        )
        .await
    }
}

#[derive(Debug, Clone, Serialize)]
struct GuardMutationPayload {
    task_id: TaskId,
    guard: RequiredGuard,
    idempotency_key: Option<String>,
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
