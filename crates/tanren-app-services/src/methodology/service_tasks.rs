//! Core task lifecycle methods for [`MethodologyService`].

use chrono::Utc;
use serde::Serialize;
use tanren_contract::methodology::{
    AbandonTaskParams, AckResponse, CompleteTaskParams, CreateTaskParams, CreateTaskResponse,
    MarkTaskGuardSatisfiedParams, SchemaVersion, StartTaskParams,
};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::{
    MethodologyEvent, TaskAbandoned as EvTaskAbandoned, TaskAdherent, TaskAudited,
    TaskCompleted as EvTaskCompleted, TaskCreated as EvTaskCreated, TaskGateChecked,
    TaskImplemented, TaskStarted, TaskXChecked, fold_task_status,
};
use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
use tanren_domain::methodology::task::{
    LegalTransition, RequiredGuard, Task, TaskAbandonDisposition, TaskStatus, TaskTransitionKind,
};
use tanren_domain::{EntityRef, SpecId, TaskId};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};
use super::phase_events::PhaseEventAttribution;
use super::service::MethodologyService;

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct GuardBridgeOrigin<'a> {
    pub tool_name: &'a str,
    pub primary_origin_kind: PhaseEventOriginKind,
}

impl MethodologyService {
    pub async fn create_task(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
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

    pub async fn start_task(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: StartTaskParams,
    ) -> MethodologyResult<AckResponse> {
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
                Ok(AckResponse::current())
            },
        )
        .await
    }

    pub async fn complete_task(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: CompleteTaskParams,
    ) -> MethodologyResult<AckResponse> {
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
                        let tool_call_id = params
                            .idempotency_key
                            .clone()
                            .unwrap_or_else(|| format!("complete_task:{}", params.task_id));
                        self.emit_with_attribution(
                            phase,
                            MethodologyEvent::TaskImplemented(TaskImplemented {
                                task_id: params.task_id,
                                spec_id,
                                evidence_refs: params.evidence_refs,
                            }),
                            PhaseEventAttribution {
                                caused_by_tool_call_id: Some(tool_call_id.clone()),
                                origin_kind: Some(PhaseEventOriginKind::ToolPrimary),
                                tool: Some("complete_task".into()),
                            },
                        )
                        .await?;
                        if self.required_guards().is_empty() {
                            self.emit_with_attribution(
                                phase,
                                MethodologyEvent::TaskCompleted(EvTaskCompleted {
                                    task_id: params.task_id,
                                    spec_id,
                                }),
                                PhaseEventAttribution {
                                    caused_by_tool_call_id: Some(tool_call_id),
                                    origin_kind: Some(PhaseEventOriginKind::ToolDerived),
                                    tool: Some("complete_task".into()),
                                },
                            )
                            .await?;
                        }
                    }
                    LegalTransition::Idempotent => {}
                }
                Ok(AckResponse::current())
            },
        )
        .await
    }

    pub async fn abandon_task(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: AbandonTaskParams,
    ) -> MethodologyResult<AckResponse> {
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
                let reason = require_non_empty("/reason", &params.reason, Some(500))?;
                match params.disposition {
                    TaskAbandonDisposition::Replacement => {
                        if params.replacements.is_empty() {
                            return Err(MethodologyError::FieldValidation {
                                field_path: "/replacements".into(),
                                expected:
                                    "at least one replacement task id when disposition=replacement"
                                        .into(),
                                actual: "replacements=[]".into(),
                                remediation:
                                    "provide replacement task ids, or use disposition=explicit_user_discard with provenance".into(),
                            });
                        }
                        if params.explicit_user_discard_provenance.is_some() {
                            return Err(MethodologyError::FieldValidation {
                                field_path: "/explicit_user_discard_provenance".into(),
                                expected: "null when disposition=replacement".into(),
                                actual: "provided".into(),
                                remediation:
                                    "remove explicit_user_discard_provenance when using replacement disposition".into(),
                            });
                        }
                    }
                    TaskAbandonDisposition::ExplicitUserDiscard => {
                        if !params.replacements.is_empty() {
                            return Err(MethodologyError::FieldValidation {
                                field_path: "/replacements".into(),
                                expected: "empty when disposition=explicit_user_discard".into(),
                                actual: format!(
                                    "replacements has {} item(s)",
                                    params.replacements.len()
                                ),
                                remediation:
                                    "clear replacements and keep explicit_user_discard_provenance".into(),
                            });
                        }
                        if !phase.is_known(KnownPhase::ResolveBlockers) {
                            return Err(MethodologyError::FieldValidation {
                                field_path: "/disposition".into(),
                                expected:
                                    "explicit_user_discard is only legal in resolve-blockers phase"
                                        .into(),
                                actual: phase.as_str().into(),
                                remediation:
                                    "run explicit user discard through resolve-blockers and pass typed provenance".into(),
                            });
                        }
                        if params.explicit_user_discard_provenance.is_none() {
                            return Err(MethodologyError::FieldValidation {
                                field_path: "/explicit_user_discard_provenance".into(),
                                expected:
                                    "non-null provenance when disposition=explicit_user_discard"
                                        .into(),
                                actual: "null".into(),
                                remediation:
                                    "set explicit_user_discard_provenance.kind=resolve_blockers with a resolution note".into(),
                            });
                        }
                    }
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
                                disposition: params.disposition,
                                replacements: params.replacements,
                                explicit_user_discard_provenance: params
                                    .explicit_user_discard_provenance,
                            }),
                        )
                        .await?;
                    }
                    LegalTransition::Idempotent => {}
                }
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// Fold current task status and validate the transition against domain rules.
    pub(crate) async fn check_transition(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
        kind: TaskTransitionKind,
    ) -> MethodologyResult<LegalTransition> {
        let current = self.current_task_status(spec_id, task_id).await?;
        current
            .legal_next(kind)
            .map_err(|e| MethodologyError::IllegalTaskTransition {
                task_id,
                from: e.from.to_owned(),
                attempted: e.attempted.to_owned(),
            })
    }

    pub(crate) async fn emit_guard_and_complete_if_converged(
        &self,
        phase: &PhaseId,
        spec_id: SpecId,
        task_id: TaskId,
        guard: RequiredGuard,
        idempotency_key: Option<String>,
        origin: GuardBridgeOrigin<'_>,
    ) -> MethodologyResult<()> {
        let tool_call_id = idempotency_key
            .clone()
            .unwrap_or_else(|| format!("{}:{task_id}:{guard:?}", origin.tool_name));
        match self
            .check_transition(spec_id, task_id, TaskTransitionKind::Guard)
            .await?
        {
            LegalTransition::Idempotent => return Ok(()),
            LegalTransition::Transition => {}
        }
        self.emit_with_attribution(
            phase,
            guard_event(task_id, spec_id, guard.clone(), idempotency_key.clone())?,
            PhaseEventAttribution {
                caused_by_tool_call_id: Some(tool_call_id.clone()),
                origin_kind: Some(origin.primary_origin_kind),
                tool: Some(origin.tool_name.to_owned()),
            },
        )
        .await?;
        let status = self.current_task_status(spec_id, task_id).await?;
        if let TaskStatus::Implemented { guards } = status
            && guards.satisfies(self.required_guards())
        {
            self.emit_with_attribution(
                phase,
                MethodologyEvent::TaskCompleted(EvTaskCompleted { task_id, spec_id }),
                PhaseEventAttribution {
                    caused_by_tool_call_id: Some(tool_call_id),
                    origin_kind: Some(PhaseEventOriginKind::ToolDerived),
                    tool: Some(origin.tool_name.to_owned()),
                },
            )
            .await?;
        }
        Ok(())
    }

    /// Emit one guard event and bridge to `TaskCompleted` when guards converge.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn mark_task_guard_satisfied(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        task_id: TaskId,
        guard: RequiredGuard,
        idempotency_key: Option<String>,
    ) -> MethodologyResult<AckResponse> {
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
                self.emit_guard_and_complete_if_converged(
                    phase,
                    spec_id,
                    task_id,
                    guard.clone(),
                    idempotency_key.clone(),
                    GuardBridgeOrigin {
                        tool_name: "mark_task_guard_satisfied",
                        primary_origin_kind: PhaseEventOriginKind::ToolPrimary,
                    },
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// Param-struct wrapper so transports can dispatch from a single
    /// compile-time registry.
    pub async fn mark_task_guard_satisfied_with_params(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: MarkTaskGuardSatisfiedParams,
    ) -> MethodologyResult<AckResponse> {
        self.mark_task_guard_satisfied(
            scope,
            phase,
            params.task_id,
            params.guard,
            params.idempotency_key,
        )
        .await
    }

    async fn current_task_status(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
    ) -> MethodologyResult<TaskStatus> {
        if let Some(projected) = self
            .store()
            .load_methodology_task_status_projection(spec_id, task_id)
            .await?
        {
            return Ok(projected.status);
        }
        let events = tanren_store::methodology::projections::load_methodology_events_for_entity(
            self.store(),
            EntityRef::Task(task_id),
            Some(spec_id),
            METHODOLOGY_PAGE_SIZE,
        )
        .await?;
        let status = fold_task_status(task_id, self.required_guards(), events.iter())
            .unwrap_or(TaskStatus::Pending);
        self.store()
            .upsert_methodology_task_status_projection(spec_id, task_id, &status)
            .await?;
        Ok(status)
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
