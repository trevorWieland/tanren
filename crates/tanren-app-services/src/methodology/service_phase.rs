//! Phase-lifecycle tool implementations.

use tanren_domain::NonEmptyString;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::{
    MethodologyEvent, PhaseOutcomeReported, ReplyDirectiveRecorded,
};
use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
use tanren_domain::methodology::phase_outcome::{BlockedReason, PhaseOutcome};
use tanren_domain::methodology::task::RequiredGuard;

use tanren_contract::methodology::{
    AckResponse, EscalateToBlockerParams, PostReplyDirectiveParams, ReportPhaseOutcomeParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;
use super::service_tasks::GuardBridgeOrigin;

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
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "report_phase_outcome",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
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
                let session = super::errors::require_non_empty(
                    "/runtime/agent_session_id",
                    &runtime.agent_session_id,
                    Some(120),
                )?;
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
                    bridged_task = Some((task_id, guard));
                }
                self.emit_event(
                    phase,
                    MethodologyEvent::PhaseOutcomeReported(PhaseOutcomeReported {
                        spec_id: params.spec_id,
                        phase: phase.clone(),
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
