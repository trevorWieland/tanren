//! Extended tool-method implementations for [`MethodologyService`].
//!
//! Holds the remaining ~22 tool methods so `service.rs` stays well
//! under the 500-line file budget. Grouped by capability surface.
//!
//! Pattern per method:
//! 1. Enforce capability scope.
//! 2. Validate inputs.
//! 3. Emit exactly one methodology event (or delegate to a read path).
//! 4. Return the typed contract response.

use chrono::Utc;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    MethodologyEvent, NonNegotiableComplianceRecorded, PhaseOutcomeReported,
    ReplyDirectiveRecorded, RubricScoreRecorded, SignpostAdded, SignpostStatusUpdated, TaskRevised,
};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::pillar::PillarScope;
use tanren_domain::methodology::rubric::{NonNegotiableCompliance, RubricScore};
use tanren_domain::methodology::signpost::Signpost;
use tanren_domain::{NonEmptyString, SignpostId, TaskId};

use tanren_contract::methodology::{
    AddSignpostParams, AddSignpostResponse, EscalateToBlockerParams, ListTasksParams,
    PostReplyDirectiveParams, RecordNonNegotiableComplianceParams, RecordRubricScoreParams,
    ReportPhaseOutcomeParams, ReviseTaskParams, SchemaVersion, UpdateSignpostStatusParams,
};
use tanren_domain::methodology::task::Task;

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;

impl MethodologyService {
    // -- §3.1 task_revise / task_list ----------------------------------------

    /// `revise_task` — non-transitional description/acceptance update.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn revise_task(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: ReviseTaskParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::TaskRevise, phase)?;
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        let reason = super::errors::require_non_empty("/reason", &params.reason, Some(500))?;
        self.emit_event(
            phase,
            MethodologyEvent::TaskRevised(TaskRevised {
                task_id: params.task_id,
                spec_id,
                revised_description: params.revised_description,
                revised_acceptance: params.revised_acceptance,
                reason,
            }),
        )
        .await
    }

    /// `list_tasks` — read-only projection.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn list_tasks(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: ListTasksParams,
    ) -> MethodologyResult<Vec<Task>> {
        enforce(scope, ToolCapability::TaskRead, phase)?;
        let Some(spec_id) = params.spec_id else {
            return Err(MethodologyError::Validation(
                "list_tasks requires spec_id at Lane 0.5 scope".into(),
            ));
        };
        let tasks = tanren_store::methodology::projections::tasks_for_spec(
            self.store(),
            spec_id,
            self.required_guards(),
        )
        .await?;
        Ok(tasks)
    }

    // -- §3.2 record_rubric_score + non-negotiable ----------------------------

    /// `record_rubric_score` — enforces the rubric linkage invariants
    /// (score < target ⇒ findings required; score < passing ⇒ at least
    /// one `fix_now` finding).
    ///
    /// # Errors
    /// See [`MethodologyError`]; the invariant check surfaces as
    /// `Validation` / `Domain`.
    pub async fn record_rubric_score(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: RecordRubricScoreParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::RubricRecord, phase)?;
        let rationale =
            super::errors::require_non_empty("/rationale", &params.rationale, Some(2000))?;
        let task_scope_target = validate_rubric_scope(&params)?;
        let pillar_tag = params.pillar.as_str().to_owned();
        let score_value = params.score.get();
        let record = RubricScore::try_new(
            params.pillar.clone(),
            params.score,
            params.target,
            params.passing,
            rationale,
            params.supporting_finding_ids.clone(),
        )
        .map_err(|e| MethodologyError::RubricInvariantViolated {
            pillar: pillar_tag,
            score: score_value,
            reason: e.to_string(),
        })?;
        let referenced = self
            .load_findings(&record.supporting_finding_ids, params.spec_id)
            .await?;
        validate_supporting_findings(&params, &record, &referenced, task_scope_target)?;
        if record.score < record.passing
            && !referenced
                .iter()
                .any(|f| matches!(f.severity, FindingSeverity::FixNow))
        {
            return Err(MethodologyError::RubricInvariantViolated {
                pillar: record.pillar.as_str().to_owned(),
                score: record.score.get(),
                reason: format!(
                    "score {} < passing {} requires at least one `fix_now` supporting finding",
                    record.score.get(),
                    record.passing.get()
                ),
            });
        }
        self.emit_event(
            phase,
            MethodologyEvent::RubricScoreRecorded(RubricScoreRecorded {
                spec_id: params.spec_id,
                scope: params.scope,
                scope_target_id: params.scope_target_id,
                score: record,
            }),
        )
        .await
    }

    /// `record_non_negotiable_compliance`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn record_non_negotiable_compliance(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: RecordNonNegotiableComplianceParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::ComplianceRecord, phase)?;
        let name = super::errors::require_non_empty("/name", &params.name, Some(120))?;
        let rationale =
            super::errors::require_non_empty("/rationale", &params.rationale, Some(2000))?;
        self.emit_event(
            phase,
            MethodologyEvent::NonNegotiableComplianceRecorded(NonNegotiableComplianceRecorded {
                spec_id: params.spec_id,
                scope: params.scope,
                compliance: NonNegotiableCompliance {
                    name,
                    status: params.status,
                    rationale,
                },
            }),
        )
        .await
    }

    // -- §3.5 signposts -------------------------------------------------------

    /// `add_signpost`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn add_signpost(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: AddSignpostParams,
    ) -> MethodologyResult<AddSignpostResponse> {
        enforce(scope, ToolCapability::SignpostAdd, phase)?;
        let problem = super::errors::require_non_empty("/problem", &params.problem, Some(500))?;
        let evidence = super::errors::require_non_empty("/evidence", &params.evidence, None)?;
        let now = Utc::now();
        let signpost = Signpost {
            id: SignpostId::new(),
            spec_id: params.spec_id,
            task_id: params.task_id,
            status: params.status,
            problem,
            evidence,
            tried: params.tried,
            solution: None,
            resolution: None,
            files_affected: params.files_affected,
            created_at: now,
            updated_at: now,
        };
        let id = signpost.id;
        self.emit_event(
            phase,
            MethodologyEvent::SignpostAdded(SignpostAdded {
                signpost: Box::new(signpost),
            }),
        )
        .await?;
        if let Ok(mut cache) = self.signpost_spec_cache.lock() {
            cache.insert(id, params.spec_id);
        }
        Ok(AddSignpostResponse {
            schema_version: SchemaVersion::current(),
            signpost_id: id,
        })
    }

    /// `update_signpost_status`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn update_signpost_status(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: UpdateSignpostStatusParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::SignpostUpdate, phase)?;
        let spec_id = self.resolve_spec_for_signpost(params.signpost_id).await?;
        self.emit_event(
            phase,
            MethodologyEvent::SignpostStatusUpdated(SignpostStatusUpdated {
                signpost_id: params.signpost_id,
                spec_id,
                status: params.status,
                resolution: params.resolution,
            }),
        )
        .await
    }

    // -- §3.6 phase outcome + escalate + post_reply ---------------------------

    /// `report_phase_outcome`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn report_phase_outcome(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: ReportPhaseOutcomeParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::PhaseOutcome, phase)?;
        let phase_name = super::errors::require_non_empty("/phase", &params.phase, Some(120))?;
        let session = super::errors::require_non_empty(
            "/agent_session_id",
            &params.agent_session_id,
            Some(120),
        )?;
        self.emit_event(
            phase,
            MethodologyEvent::PhaseOutcomeReported(PhaseOutcomeReported {
                spec_id: params.spec_id,
                phase: phase_name,
                agent_session_id: session,
                outcome: params.outcome,
            }),
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
        phase: &str,
        params: EscalateToBlockerParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::PhaseEscalate, phase)?;
        if phase != "investigate" {
            return Err(MethodologyError::FieldValidation {
                field_path: "/phase".into(),
                expected: "escalate_to_blocker allowed only in investigate".into(),
                actual: phase.to_owned(),
                remediation: "invoke escalate_to_blocker from investigate only".into(),
            });
        }
        let reason = super::errors::require_non_empty("/reason", &params.reason, Some(1000))?;
        let summary = NonEmptyString::try_new(format!(
            "escalated: {} options={}",
            reason.as_str(),
            params.options.len()
        ))
        .map_err(|e| MethodologyError::Internal(e.to_string()))?;
        let phase_name = super::errors::require_non_empty("/phase", phase, Some(120))?;
        self.emit_event(
            phase,
            MethodologyEvent::PhaseOutcomeReported(PhaseOutcomeReported {
                spec_id: params.spec_id,
                phase: phase_name,
                agent_session_id: NonEmptyString::try_new("escalation")
                    .map_err(|e| MethodologyError::Internal(e.to_string()))?,
                outcome: tanren_domain::methodology::phase_outcome::PhaseOutcome::Blocked {
                    reason: tanren_domain::methodology::phase_outcome::BlockedReason::Other {
                        detail: reason,
                    },
                    summary,
                },
            }),
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
        phase: &str,
        params: PostReplyDirectiveParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::FeedbackReply, phase)?;
        if phase != "handle-feedback" {
            return Err(MethodologyError::FieldValidation {
                field_path: "/phase".into(),
                expected: "post_reply_directive allowed only in handle-feedback".into(),
                actual: phase.to_owned(),
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
                    "supply the reply content the orchestrator's feedback adapter will post".into(),
            });
        } else {
            params.body
        };
        let phase_name = super::errors::require_non_empty("/phase", phase, Some(120))?;
        self.emit_event(
            phase,
            MethodologyEvent::ReplyDirectiveRecorded(ReplyDirectiveRecorded {
                spec_id: params.spec_id,
                phase: phase_name,
                thread_ref,
                disposition: params.disposition,
                body,
            }),
        )
        .await
    }
}

fn validate_rubric_scope(params: &RecordRubricScoreParams) -> MethodologyResult<Option<TaskId>> {
    match params.scope {
        PillarScope::Spec => {
            if params.scope_target_id.is_some() {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/scope_target_id".into(),
                    expected: "absent for spec-scoped rubric scores".into(),
                    actual: format!("{:?}", params.scope_target_id),
                    remediation: "omit scope_target_id when scope=spec".into(),
                });
            }
            Ok(None)
        }
        PillarScope::Task => {
            let Some(raw) = params.scope_target_id.as_deref() else {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/scope_target_id".into(),
                    expected: "task id string when scope=task".into(),
                    actual: "null".into(),
                    remediation: "set scope_target_id to the task id this rubric score evaluates"
                        .into(),
                });
            };
            let uuid =
                uuid::Uuid::parse_str(raw).map_err(|e| MethodologyError::FieldValidation {
                    field_path: "/scope_target_id".into(),
                    expected: "UUID task id".into(),
                    actual: raw.to_owned(),
                    remediation: e.to_string(),
                })?;
            Ok(Some(TaskId::from_uuid(uuid)))
        }
    }
}

fn validate_supporting_findings(
    params: &RecordRubricScoreParams,
    record: &RubricScore,
    findings: &[Finding],
    task_scope_target: Option<TaskId>,
) -> MethodologyResult<()> {
    for (idx, finding) in findings.iter().enumerate() {
        let source_pillar = match &finding.source {
            FindingSource::Audit {
                pillar: Some(pillar),
                ..
            } => pillar.as_str(),
            FindingSource::Audit { pillar: None, .. } => {
                return Err(MethodologyError::FieldValidation {
                    field_path: format!("/supporting_finding_ids/{idx}"),
                    expected: "audit finding with matching pillar".into(),
                    actual: format!("finding {} has no pillar", finding.id),
                    remediation:
                        "record the finding with source.audit.pillar matching the rubric pillar"
                            .into(),
                });
            }
            _ => {
                return Err(MethodologyError::FieldValidation {
                    field_path: format!("/supporting_finding_ids/{idx}"),
                    expected: "audit-sourced finding".into(),
                    actual: format!("finding {} source is not audit", finding.id),
                    remediation:
                        "support rubric scores with audit findings linked to the same pillar".into(),
                });
            }
        };
        if source_pillar != record.pillar.as_str() {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/supporting_finding_ids/{idx}"),
                expected: format!("pillar `{}`", record.pillar.as_str()),
                actual: source_pillar.to_owned(),
                remediation:
                    "link only findings whose source.audit.pillar matches the scored pillar".into(),
            });
        }
        if !matches!(
            finding.severity,
            FindingSeverity::FixNow | FindingSeverity::Defer
        ) {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/supporting_finding_ids/{idx}"),
                expected: "severity fix_now or defer".into(),
                actual: finding.severity.to_string(),
                remediation: "use supporting findings with actionable severities (fix_now/defer)"
                    .into(),
            });
        }
        if let Some(task_id) = task_scope_target
            && finding.attached_task != Some(task_id)
        {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/supporting_finding_ids/{idx}"),
                expected: format!("finding attached_task={task_id}"),
                actual: format!("{:?}", finding.attached_task),
                remediation:
                    "for task-scoped scores, support with findings attached to the same task".into(),
            });
        }
    }
    if params.scope == PillarScope::Task && record.score < record.target && findings.is_empty() {
        return Err(MethodologyError::RubricInvariantViolated {
            pillar: record.pillar.as_str().to_owned(),
            score: record.score.get(),
            reason: "task-scoped score below target requires supporting findings".into(),
        });
    }
    Ok(())
}
