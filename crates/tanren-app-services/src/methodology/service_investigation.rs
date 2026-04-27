use chrono::Utc;
use tanren_contract::methodology::{
    AckResponse, LinkRootCauseToFindingParams, ListInvestigationAttemptsParams,
    ListInvestigationAttemptsResponse, RecordInvestigationAttemptParams,
    RecordInvestigationAttemptResponse, SchemaVersion,
};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{
    InvestigationAttemptRecorded, MethodologyEvent, RootCauseLinkedToFinding,
};
use tanren_domain::methodology::investigation::{InvestigationAttempt, InvestigationRootCause};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::{InvestigationAttemptId, RootCauseId};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;

impl MethodologyService {
    pub async fn list_investigation_attempts(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: ListInvestigationAttemptsParams,
    ) -> MethodologyResult<ListInvestigationAttemptsResponse> {
        enforce(scope, ToolCapability::InvestigationRecord, phase)?;
        let fingerprint =
            super::errors::require_non_empty("/fingerprint", &params.fingerprint, Some(240))?;
        let mut attempts = tanren_store::methodology::investigation_attempts_for_spec(
            self.store(),
            params.spec_id,
        )
        .await?;
        attempts.retain(|attempt| attempt.fingerprint == fingerprint);
        if let Some(source_check) = params.source_check {
            attempts.retain(|attempt| attempt.source_check == source_check);
        }
        if let Some(finding_id) = params.finding_id {
            attempts.retain(|attempt| attempt.source_findings.contains(&finding_id));
        }
        Ok(ListInvestigationAttemptsResponse {
            schema_version: SchemaVersion::current(),
            attempts,
        })
    }

    pub async fn record_investigation_attempt(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: RecordInvestigationAttemptParams,
    ) -> MethodologyResult<RecordInvestigationAttemptResponse> {
        enforce(scope, ToolCapability::InvestigationRecord, phase)?;
        self.ensure_findings_exist(params.spec_id, &params.source_findings)
            .await?;
        if let Some(task_id) = params.source_check.scope.task_id() {
            let task_spec_id = self.resolve_spec_for_task(task_id).await?;
            if task_spec_id != params.spec_id {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/source_check/scope/task_id".into(),
                    expected: format!("task in spec {}", params.spec_id),
                    actual: format!("task in spec {task_spec_id}"),
                    remediation: "record investigation attempts against one spec".into(),
                });
            }
        }
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "record_investigation_attempt",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let attempt_id = InvestigationAttemptId::new();
                let mut root_cause_ids = Vec::with_capacity(params.root_causes.len());
                let mut root_causes = Vec::with_capacity(params.root_causes.len());
                for (idx, input) in params.root_causes.into_iter().enumerate() {
                    let id = RootCauseId::new();
                    root_cause_ids.push(id);
                    root_causes.push(InvestigationRootCause {
                        id,
                        description: super::errors::require_non_empty(
                            &format!("/root_causes/{idx}/description"),
                            &input.description,
                            Some(1000),
                        )?,
                        confidence: input.confidence,
                        category: input.category,
                        affected_files: input.affected_files,
                    });
                }
                let attempt = InvestigationAttempt {
                    id: attempt_id,
                    spec_id: params.spec_id,
                    fingerprint: super::errors::require_non_empty(
                        "/fingerprint",
                        &params.fingerprint,
                        Some(240),
                    )?,
                    loop_index: params.loop_index,
                    source_check: params.source_check,
                    source_findings: params.source_findings,
                    evidence_refs: params.evidence_refs,
                    root_causes,
                    recorded_at: Utc::now(),
                };
                self.emit(
                    phase,
                    MethodologyEvent::InvestigationAttemptRecorded(InvestigationAttemptRecorded {
                        attempt,
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(RecordInvestigationAttemptResponse {
                    schema_version: SchemaVersion::current(),
                    attempt_id,
                    root_cause_ids,
                })
            },
        )
        .await
    }

    pub async fn link_root_cause_to_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: LinkRootCauseToFindingParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::InvestigationRecord, phase)?;
        self.ensure_findings_exist(params.spec_id, &[params.finding_id])
            .await?;
        self.ensure_attempt_root_cause(params.spec_id, params.attempt_id, params.root_cause_id)
            .await?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "link_root_cause_to_finding",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit(
                    phase,
                    MethodologyEvent::RootCauseLinkedToFinding(RootCauseLinkedToFinding {
                        spec_id: params.spec_id,
                        attempt_id: params.attempt_id,
                        root_cause_id: params.root_cause_id,
                        finding_id: params.finding_id,
                        source_check: params.source_check,
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    async fn ensure_attempt_root_cause(
        &self,
        spec_id: tanren_domain::SpecId,
        attempt_id: InvestigationAttemptId,
        root_cause_id: RootCauseId,
    ) -> MethodologyResult<InvestigationAttempt> {
        let Some(attempt) = tanren_store::methodology::investigation_attempt_by_id(
            self.store(),
            spec_id,
            attempt_id,
        )
        .await?
        else {
            return Err(MethodologyError::FieldValidation {
                field_path: "/attempt_id".into(),
                expected: "investigation attempt in this spec".into(),
                actual: attempt_id.to_string(),
                remediation: "record an investigation attempt before linking provenance".into(),
            });
        };
        if !attempt
            .root_causes
            .iter()
            .any(|root_cause| root_cause.id == root_cause_id)
        {
            return Err(MethodologyError::FieldValidation {
                field_path: "/root_cause_id".into(),
                expected: format!("root cause in attempt {attempt_id}"),
                actual: root_cause_id.to_string(),
                remediation: "link only root causes emitted by the referenced attempt".into(),
            });
        }
        Ok(attempt)
    }
}
