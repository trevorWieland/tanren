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
    MethodologyEvent, NonNegotiableComplianceRecorded, PhaseOutcomeReported, RubricScoreRecorded,
    SignpostAdded, SignpostStatusUpdated, TaskRevised,
};
use tanren_domain::methodology::finding::FindingSeverity;
use tanren_domain::methodology::rubric::{NonNegotiableCompliance, RubricScore};
use tanren_domain::methodology::signpost::Signpost;
use tanren_domain::{NonEmptyString, SignpostId};

use tanren_contract::methodology::{
    AddSignpostParams, AddSignpostResponse, EscalateToBlockerParams, ListTasksParams,
    PostReplyDirectiveParams, RecordNonNegotiableComplianceParams, RecordRubricScoreParams,
    ReportPhaseOutcomeParams, ReviseTaskParams, UpdateSignpostStatusParams,
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
        let reason = NonEmptyString::try_new(params.reason)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        self.emit_event(MethodologyEvent::TaskRevised(TaskRevised {
            task_id: params.task_id,
            spec_id,
            revised_description: params.revised_description,
            revised_acceptance: params.revised_acceptance,
            reason,
        }))
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
        let required = [
            tanren_domain::methodology::task::RequiredGuard::GateChecked,
            tanren_domain::methodology::task::RequiredGuard::Audited,
            tanren_domain::methodology::task::RequiredGuard::Adherent,
        ];
        let tasks = tanren_store::methodology::projections::tasks_for_spec(
            self.store(),
            spec_id,
            &required,
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
        let rationale = NonEmptyString::try_new(params.rationale)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        let record = RubricScore::try_new(
            params.pillar,
            params.score,
            params.target,
            params.passing,
            rationale,
            params.supporting_finding_ids,
        )?;
        if record.score < record.passing {
            let referenced = self
                .load_findings(&record.supporting_finding_ids, params.spec_id)
                .await?;
            let has_fix_now = referenced
                .iter()
                .any(|f| matches!(f.severity, FindingSeverity::FixNow));
            if !has_fix_now {
                return Err(MethodologyError::Validation(format!(
                    "pillar {}: score {} < passing {} requires at least one `fix_now` supporting finding",
                    record.pillar.as_str(),
                    record.score.get(),
                    record.passing.get()
                )));
            }
        }
        self.emit_event(MethodologyEvent::RubricScoreRecorded(RubricScoreRecorded {
            spec_id: params.spec_id,
            scope: params.scope,
            scope_target_id: params.scope_target_id,
            score: record,
        }))
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
        let name = NonEmptyString::try_new(params.name)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        let rationale = NonEmptyString::try_new(params.rationale)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        self.emit_event(MethodologyEvent::NonNegotiableComplianceRecorded(
            NonNegotiableComplianceRecorded {
                spec_id: params.spec_id,
                scope: params.scope,
                compliance: NonNegotiableCompliance {
                    name,
                    status: params.status,
                    rationale,
                },
            },
        ))
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
        let problem = NonEmptyString::try_new(params.problem)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        let evidence = NonEmptyString::try_new(params.evidence)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
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
        self.emit_event(MethodologyEvent::SignpostAdded(SignpostAdded {
            signpost: Box::new(signpost),
        }))
        .await?;
        Ok(AddSignpostResponse { signpost_id: id })
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
        self.emit_event(MethodologyEvent::SignpostStatusUpdated(
            SignpostStatusUpdated {
                signpost_id: params.signpost_id,
                spec_id,
                status: params.status,
                resolution: params.resolution,
            },
        ))
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
        let phase_name = NonEmptyString::try_new(params.phase)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        let session = NonEmptyString::try_new(params.agent_session_id)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        self.emit_event(MethodologyEvent::PhaseOutcomeReported(
            PhaseOutcomeReported {
                spec_id: params.spec_id,
                phase: phase_name,
                agent_session_id: session,
                outcome: params.outcome,
            },
        ))
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
        let reason = NonEmptyString::try_new(params.reason)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        let summary = NonEmptyString::try_new(format!(
            "escalated: {} options={}",
            reason.as_str(),
            params.options.len()
        ))
        .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        self.emit_event(MethodologyEvent::PhaseOutcomeReported(
            PhaseOutcomeReported {
                spec_id: params.spec_id,
                phase: NonEmptyString::try_new(phase)
                    .map_err(|e| MethodologyError::Validation(e.to_string()))?,
                agent_session_id: NonEmptyString::try_new("escalation")
                    .map_err(|e| MethodologyError::Validation(e.to_string()))?,
                outcome: tanren_domain::methodology::phase_outcome::PhaseOutcome::Blocked {
                    reason: tanren_domain::methodology::phase_outcome::BlockedReason::Other {
                        detail: reason,
                    },
                    summary,
                },
            },
        ))
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
        // No dedicated event variant yet — record as a PhaseOutcome
        // Complete with a structured summary for the orchestrator to
        // pick up. Dedicated variant lands in a follow-up lane.
        let summary = NonEmptyString::try_new(format!(
            "feedback:{}:{}",
            params.thread_ref,
            serde_json::to_string(&params.disposition).unwrap_or_default()
        ))
        .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        self.emit_event(MethodologyEvent::PhaseOutcomeReported(
            PhaseOutcomeReported {
                spec_id: params.spec_id,
                phase: NonEmptyString::try_new(phase)
                    .map_err(|e| MethodologyError::Validation(e.to_string()))?,
                agent_session_id: NonEmptyString::try_new("reply-directive")
                    .map_err(|e| MethodologyError::Validation(e.to_string()))?,
                outcome: tanren_domain::methodology::phase_outcome::PhaseOutcome::Complete {
                    summary,
                    next_action_hint: None,
                },
            },
        ))
        .await?;
        drop(params.body);
        Ok(())
    }
}
