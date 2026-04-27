//! Phase-lifecycle tool implementations.

use tanren_domain::NonEmptyString;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::{
    MethodologyEvent, PhaseOutcomeReported, ReplyDirectiveRecorded,
};
use tanren_domain::methodology::finding::FindingSource;
use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
use tanren_domain::methodology::phase_outcome::{BlockedReason, PhaseOutcome};
use tanren_domain::methodology::task::{RequiredGuard, TaskStatus};

use tanren_contract::methodology::{
    AckResponse, EscalateToBlockerParams, PostReplyDirectiveParams, ReportPhaseOutcomeParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;
use super::service_tasks::GuardBridgeOrigin;

#[derive(serde::Serialize)]
struct ScopedPhaseOutcomePayload {
    phase: String,
    params: ReportPhaseOutcomeParams,
}

fn task_check_source_matches(phase: &PhaseId, source: &FindingSource) -> bool {
    matches!(
        (phase.known(), source),
        (Some(KnownPhase::AuditTask), FindingSource::Audit { .. })
            | (
                Some(KnownPhase::AdhereTask),
                FindingSource::Adherence { .. }
            )
    )
}

fn spec_check_source_matches(phase: &PhaseId, source: &FindingSource) -> bool {
    matches!(
        (phase.known(), source),
        (Some(KnownPhase::AuditSpec), FindingSource::Audit { .. })
            | (
                Some(KnownPhase::AdhereSpec),
                FindingSource::Adherence { .. }
            )
    )
}

impl MethodologyService {
    /// `report_phase_outcome`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn report_phase_outcome(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: ReportPhaseOutcomeParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::PhaseOutcome, phase)?;
        let spec_id = params.spec_id;
        validate_task_scoped_phase_task_id(self, phase, params.spec_id, params.task_id).await?;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = ScopedPhaseOutcomePayload {
            phase: phase.as_str().to_owned(),
            params: params.clone(),
        };
        self.run_idempotent_mutation(
            "report_phase_outcome",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let session = self.phase_runtime_session_for_spec(params.spec_id)?;
                let guard_to_bridge = match phase.known() {
                    Some(KnownPhase::AuditTask) => Some(RequiredGuard::Audited),
                    Some(KnownPhase::AdhereTask) => Some(RequiredGuard::Adherent),
                    _ => None,
                };
                let mut bridged_task = None;
                if let Some(guard) = guard_to_bridge
                    && matches!(params.outcome, PhaseOutcome::Complete { .. })
                {
                    let task_id = params.task_id.ok_or_else(|| MethodologyError::FieldValidation {
                        field_path: "/task_id".into(),
                        expected: "task_id is required for task-scoped phase completion".into(),
                        actual: "missing".into(),
                        remediation:
                            "provide the audited/adhered task_id on report_phase_outcome".into(),
                    })?;
                    let task_spec_id = self.resolve_spec_for_task(task_id).await?;
                    if task_spec_id != params.spec_id {
                        return Err(MethodologyError::FieldValidation {
                            field_path: "/task_id".into(),
                            expected: format!("task in spec {}", params.spec_id),
                            actual: format!("task in spec {task_spec_id}"),
                            remediation:
                            "report task-scoped outcomes with a task_id in the same spec"
                                    .into(),
                        });
                    }
                    self.reject_open_blocking_task_findings(phase, params.spec_id, task_id)
                        .await?;
                    if !matches!(
                        self.current_task_status(params.spec_id, task_id).await?,
                        TaskStatus::Complete
                    ) {
                        bridged_task = Some((task_id, guard));
                    }
                }
                if matches!(params.outcome, PhaseOutcome::Complete { .. })
                    && matches!(
                        phase.known(),
                        Some(KnownPhase::AuditSpec | KnownPhase::AdhereSpec)
                    )
                {
                    self.reject_open_blocking_spec_findings(phase, params.spec_id)
                        .await?;
                }
                self.emit_event(
                    phase,
                    MethodologyEvent::PhaseOutcomeReported(PhaseOutcomeReported {
                        spec_id: params.spec_id,
                        phase: phase.clone(),
                        task_id: params.task_id,
                        agent_session_id: session,
                        outcome: params.outcome.clone(),
                    }),
                )
                .await?;
                if let Some((task_id, guard)) = bridged_task {
                    self.emit_guard_and_complete_if_converged(
                        phase,
                        params.spec_id,
                        task_id,
                        guard,
                        params.idempotency_key.clone(),
                        GuardBridgeOrigin {
                            tool_name: "report_phase_outcome",
                            primary_origin_kind: PhaseEventOriginKind::ToolDerived,
                        },
                    )
                    .await?;
                }
                Ok(AckResponse::current())
            },
        )
        .await
    }

    fn phase_runtime_session_for_spec(
        &self,
        spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<NonEmptyString> {
        let runtime = self
            .phase_events_runtime()
            .ok_or_else(|| MethodologyError::FieldValidation {
                field_path: "/spec_id".into(),
                expected: "audited runtime requires canonical spec binding".into(),
                actual: "missing runtime".into(),
                remediation:
                    "set --spec-id/--spec-folder or TANREN_SPEC_ID/TANREN_SPEC_FOLDER for mutating methodology calls"
                        .into(),
            })?;
        if runtime.spec_id != spec_id {
            return Err(MethodologyError::FieldValidation {
                field_path: "/spec_id".into(),
                expected: runtime.spec_id.to_string(),
                actual: spec_id.to_string(),
                remediation: "use the canonical session spec_id for all mutation tool calls".into(),
            });
        }
        super::errors::require_non_empty(
            "/runtime/agent_session_id",
            &runtime.agent_session_id,
            Some(120),
        )
    }

    async fn reject_open_blocking_task_findings(
        &self,
        phase: &PhaseId,
        spec_id: tanren_domain::SpecId,
        task_id: tanren_domain::TaskId,
    ) -> MethodologyResult<()> {
        let findings =
            tanren_store::methodology::finding_views_for_spec(self.store(), spec_id).await?;
        let blocking = findings
            .iter()
            .filter(|view| view.is_open_blocking())
            .filter(|view| view.finding.attached_task == Some(task_id))
            .filter(|view| task_check_source_matches(phase, &view.finding.source))
            .map(|view| view.finding.id.to_string())
            .collect::<Vec<_>>();
        if blocking.is_empty() {
            return Ok(());
        }
        Err(MethodologyError::FieldValidation {
            field_path: "/outcome".into(),
            expected: "no open blocking findings for task check scope".into(),
            actual: format!("open finding ids: {}", blocking.join(", ")),
            remediation: "resolve, defer, or supersede blocking findings before reporting complete"
                .into(),
        })
    }

    async fn reject_open_blocking_spec_findings(
        &self,
        phase: &PhaseId,
        spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<()> {
        let findings =
            tanren_store::methodology::finding_views_for_spec(self.store(), spec_id).await?;
        let blocking = findings
            .iter()
            .filter(|view| view.is_open_blocking())
            .filter(|view| view.finding.attached_task.is_none())
            .filter(|view| spec_check_source_matches(phase, &view.finding.source))
            .map(|view| view.finding.id.to_string())
            .collect::<Vec<_>>();
        if blocking.is_empty() {
            return Ok(());
        }
        Err(MethodologyError::FieldValidation {
            field_path: "/outcome".into(),
            expected: "no open blocking findings for spec check scope".into(),
            actual: format!("open finding ids: {}", blocking.join(", ")),
            remediation: "resolve, defer, or supersede blocking findings before reporting complete"
                .into(),
        })
    }

    /// `escalate_to_blocker` — capability-scoped to `investigate` at
    /// phase config time.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn escalate_to_blocker(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: EscalateToBlockerParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::PhaseEscalate, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "escalate_to_blocker",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                if !phase.is_known(KnownPhase::Investigate) {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/phase".into(),
                        expected: "escalate_to_blocker allowed only in investigate".into(),
                        actual: phase.as_str().to_owned(),
                        remediation: "invoke escalate_to_blocker from investigate only".into(),
                    });
                }
                let reason = super::errors::require_non_empty("/reason", &params.reason, Some(1000))?;
                let options: Vec<String> = params
                    .options
                    .iter()
                    .map(|option| option.trim())
                    .filter(|option| !option.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
                if options.is_empty() {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/options".into(),
                        expected: "at least one non-empty option".into(),
                        actual: format!("{:?}", params.options),
                        remediation:
                            "provide one or more actionable options for resolve-blockers".into(),
                    });
                }
                let runtime =
                    self.phase_events_runtime()
                        .ok_or_else(|| MethodologyError::FieldValidation {
                            field_path: "/spec_id".into(),
                            expected: "audited runtime requires canonical spec binding".into(),
                            actual: "missing runtime".into(),
                            remediation:
                                "set --spec-id/--spec-folder or TANREN_SPEC_ID/TANREN_SPEC_FOLDER for mutating methodology calls".into(),
                        })?;
                if runtime.spec_id != params.spec_id {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/spec_id".into(),
                        expected: runtime.spec_id.to_string(),
                        actual: params.spec_id.to_string(),
                        remediation:
                            "use the canonical session spec_id for all mutation tool calls".into(),
                    });
                }
                let options_inline = options.join(" | ");
                let summary = NonEmptyString::try_new(format!(
                    "investigate escalation: {} (options: {})",
                    reason.as_str(),
                    options_inline
                ))
                .map_err(|e| MethodologyError::Internal(e.to_string()))?;
                let mut prompt = format!("reason: {}\noptions:", reason.as_str());
                for option in &options {
                    prompt.push_str("\n- ");
                    prompt.push_str(option);
                }
                let prompt = NonEmptyString::try_new(prompt)
                    .map_err(|e| MethodologyError::Internal(e.to_string()))?;
                self.emit_event(
                    phase,
                    MethodologyEvent::PhaseOutcomeReported(PhaseOutcomeReported {
                        spec_id: params.spec_id,
                        phase: phase.clone(),
                        task_id: None,
                        agent_session_id: super::errors::require_non_empty(
                            "/runtime/agent_session_id",
                            &runtime.agent_session_id,
                            Some(120),
                        )?,
                        outcome: PhaseOutcome::Blocked {
                            reason: BlockedReason::AwaitingHumanInput { prompt },
                            summary,
                        },
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// `post_reply_directive` — capability-scoped to `handle-feedback`.
    /// Records the disposition on a feedback thread; the orchestrator
    /// enacts the actual reply out-of-band.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn post_reply_directive(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: PostReplyDirectiveParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::FeedbackReply, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "post_reply_directive",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                if !phase.is_known(KnownPhase::HandleFeedback) {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/phase".into(),
                        expected: "post_reply_directive allowed only in handle-feedback".into(),
                        actual: phase.as_str().to_owned(),
                        remediation: "invoke post_reply_directive from handle-feedback only".into(),
                    });
                }
                let thread_ref =
                    super::errors::require_non_empty("/thread_ref", &params.thread_ref, Some(200))?;
                let body = if params.body.trim().is_empty() {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/body".into(),
                        expected: "non-empty reply body".into(),
                        actual: format!("{:?}", params.body),
                        remediation:
                            "supply the reply content the orchestrator's feedback adapter will post"
                                .into(),
                    });
                } else {
                    params.body
                };
                self.emit_event(
                    phase,
                    MethodologyEvent::ReplyDirectiveRecorded(ReplyDirectiveRecorded {
                        spec_id: params.spec_id,
                        phase: phase.clone(),
                        thread_ref,
                        disposition: params.disposition,
                        body,
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }
}

async fn validate_task_scoped_phase_task_id(
    service: &MethodologyService,
    phase: &PhaseId,
    spec_id: tanren_domain::SpecId,
    task_id: Option<tanren_domain::TaskId>,
) -> MethodologyResult<()> {
    if !matches!(
        phase.known(),
        Some(KnownPhase::DoTask | KnownPhase::AuditTask | KnownPhase::AdhereTask)
    ) {
        return Ok(());
    }
    let Some(task_id) = task_id else {
        return Err(MethodologyError::FieldValidation {
            field_path: "/task_id".into(),
            expected: "task_id for task-scoped phase outcome".into(),
            actual: "missing".into(),
            remediation: "task-loop phase outcomes must identify the source task".into(),
        });
    };
    let task_spec_id = service.resolve_spec_for_task(task_id).await?;
    if task_spec_id == spec_id {
        return Ok(());
    }
    Err(MethodologyError::FieldValidation {
        field_path: "/task_id".into(),
        expected: format!("task in spec {spec_id}"),
        actual: format!("task in spec {task_spec_id}"),
        remediation: "report task-scoped outcomes with a task_id in the same spec".into(),
    })
}
