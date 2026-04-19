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
use tanren_domain::SignpostId;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    MethodologyEvent, NonNegotiableComplianceRecorded, RubricScoreRecorded, SignpostAdded,
    SignpostStatusUpdated, TaskRevised,
};
use tanren_domain::methodology::finding::FindingSeverity;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::pillar::Pillar;
use tanren_domain::methodology::rubric::{NonNegotiableCompliance, RubricScore};
use tanren_domain::methodology::signpost::Signpost;

use tanren_contract::methodology::{
    AckResponse, AddSignpostParams, AddSignpostResponse, ListTasksParams, ListTasksResponse,
    RecordNonNegotiableComplianceParams, RecordRubricScoreParams, ReviseTaskParams, SchemaVersion,
    UpdateSignpostStatusParams,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;
use super::service_ext_validation::{validate_rubric_scope, validate_supporting_findings};

impl MethodologyService {
    // -- §3.1 task_revise / task_list ----------------------------------------

    /// `revise_task` — non-transitional description/acceptance update.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn revise_task(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: ReviseTaskParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::TaskRevise, phase)?;
        let spec_id = self.resolve_spec_for_task(params.task_id).await?;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "revise_task",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let reason =
                    super::errors::require_non_empty("/reason", &params.reason, Some(500))?;
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
                .await?;
                Ok(AckResponse::current())
            },
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
        phase: &PhaseId,
        params: ListTasksParams,
    ) -> MethodologyResult<ListTasksResponse> {
        enforce(scope, ToolCapability::TaskRead, phase)?;
        let spec_id = match params.spec_id {
            Some(spec_id) => spec_id,
            None => self
                .phase_events_runtime()
                .map(|runtime| runtime.spec_id)
                .ok_or_else(|| MethodologyError::FieldValidation {
                    field_path: "/spec_id".into(),
                    expected:
                        "spec_id in params or active session runtime with canonical spec_id".into(),
                    actual: "missing".into(),
                    remediation:
                        "pass `spec_id` to list_tasks, or invoke it from a bound mutation session that sets TANREN_SPEC_ID".into(),
                })?,
        };
        let tasks = tanren_store::methodology::projections::tasks_for_spec(
            self.store(),
            spec_id,
            self.required_guards(),
        )
        .await?;
        Ok(ListTasksResponse {
            schema_version: SchemaVersion::current(),
            tasks,
        })
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
        phase: &PhaseId,
        params: RecordRubricScoreParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::RubricRecord, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "record_rubric_score",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let rationale =
                    super::errors::require_non_empty("/rationale", &params.rationale, Some(2000))?;
                let task_scope_target = validate_rubric_scope(&params)?;
                let registry_pillar = resolve_registry_pillar(self.pillars(), &params)?;
                if params.target != registry_pillar.target_score {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/target".into(),
                        expected: registry_pillar.target_score.to_string(),
                        actual: params.target.to_string(),
                        remediation:
                            "use target from the runtime rubric registry; callers cannot override it"
                                .into(),
                    });
                }
                if params.passing != registry_pillar.passing_score {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/passing".into(),
                        expected: registry_pillar.passing_score.to_string(),
                        actual: params.passing.to_string(),
                        remediation:
                            "use passing threshold from the runtime rubric registry; callers cannot override it".into(),
                    });
                }
                let pillar_tag = registry_pillar.id.as_str().to_owned();
                let score_value = params.score.get();
                let record = RubricScore::try_new(
                    registry_pillar.id.clone(),
                    params.score,
                    registry_pillar.target_score,
                    registry_pillar.passing_score,
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
                .await?;
                Ok(AckResponse::current())
            },
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
        phase: &PhaseId,
        params: RecordNonNegotiableComplianceParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::ComplianceRecord, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "record_non_negotiable_compliance",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let name = super::errors::require_non_empty("/name", &params.name, Some(120))?;
                let rationale =
                    super::errors::require_non_empty("/rationale", &params.rationale, Some(2000))?;
                self.emit_event(
                    phase,
                    MethodologyEvent::NonNegotiableComplianceRecorded(
                        NonNegotiableComplianceRecorded {
                            spec_id: params.spec_id,
                            scope: params.scope,
                            compliance: NonNegotiableCompliance {
                                name,
                                status: params.status,
                                rationale,
                            },
                        },
                    ),
                )
                .await?;
                Ok(AckResponse::current())
            },
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
        phase: &PhaseId,
        params: AddSignpostParams,
    ) -> MethodologyResult<AddSignpostResponse> {
        enforce(scope, ToolCapability::SignpostAdd, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "add_signpost",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let problem =
                    super::errors::require_non_empty("/problem", &params.problem, Some(500))?;
                let evidence =
                    super::errors::require_non_empty("/evidence", &params.evidence, None)?;
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
            },
        )
        .await
    }

    /// `update_signpost_status`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn update_signpost_status(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: UpdateSignpostStatusParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::SignpostUpdate, phase)?;
        let spec_id = self.resolve_spec_for_signpost(params.signpost_id).await?;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "update_signpost_status",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit_event(
                    phase,
                    MethodologyEvent::SignpostStatusUpdated(SignpostStatusUpdated {
                        signpost_id: params.signpost_id,
                        spec_id,
                        status: params.status,
                        resolution: params.resolution,
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }
}

fn resolve_registry_pillar<'a>(
    pillars: &'a [Pillar],
    params: &RecordRubricScoreParams,
) -> MethodologyResult<&'a Pillar> {
    let Some(pillar) = pillars.iter().find(|p| p.id == params.pillar) else {
        let known = pillars
            .iter()
            .map(|p| p.id.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(MethodologyError::FieldValidation {
            field_path: "/pillar".into(),
            expected: format!("known pillar id (one of: {known})"),
            actual: params.pillar.to_string(),
            remediation:
                "use a pillar id present in tanren/rubric.yml or methodology.rubric.pillars".into(),
        });
    };
    if !pillar.applicable_at.includes(params.scope) {
        return Err(MethodologyError::FieldValidation {
            field_path: "/scope".into(),
            expected: format!("scope compatible with pillar `{}`", pillar.id),
            actual: format!("{:?}", params.scope),
            remediation:
                "record rubric score in a scope listed by the pillar's `applicable_at` policy"
                    .into(),
        });
    }
    Ok(pillar)
}
