use chrono::Utc;
use tanren_contract::methodology::{
    AckResponse, RecordCheckResultParams, SchemaVersion, StartCheckRunParams, StartCheckRunResponse,
};
use tanren_domain::CheckRunId;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::check::{CheckResult, CheckRun, CheckStatus};
use tanren_domain::methodology::events::{
    CheckFailureRecorded, CheckResultRecorded, CheckRunStarted, MethodologyEvent,
};
use tanren_domain::methodology::phase_id::PhaseId;

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;

impl MethodologyService {
    pub async fn start_check_run(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: StartCheckRunParams,
    ) -> MethodologyResult<StartCheckRunResponse> {
        enforce(scope, ToolCapability::CheckRecord, phase)?;
        if let Some(task_id) = params.scope.task_id() {
            let task_spec_id = self.resolve_spec_for_task(task_id).await?;
            if task_spec_id != params.spec_id {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/scope/task_id".into(),
                    expected: format!("task in spec {}", params.spec_id),
                    actual: format!("task in spec {task_spec_id}"),
                    remediation: "record task-scoped checks against tasks in the same spec".into(),
                });
            }
        }
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "start_check_run",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let check = CheckRun {
                    id: CheckRunId::new(),
                    spec_id: params.spec_id,
                    kind: params.kind,
                    scope: params.scope,
                    source_phase: phase.clone(),
                    fingerprint: params
                        .fingerprint
                        .map(|raw| {
                            super::errors::require_non_empty("/fingerprint", &raw, Some(240))
                        })
                        .transpose()?,
                    started_at: Utc::now(),
                };
                let check_run_id = check.id;
                self.emit(
                    phase,
                    MethodologyEvent::CheckRunStarted(CheckRunStarted {
                        check,
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(StartCheckRunResponse {
                    schema_version: SchemaVersion::current(),
                    check_run_id,
                })
            },
        )
        .await
    }

    pub async fn record_check_result(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: RecordCheckResultParams,
    ) -> MethodologyResult<AckResponse> {
        self.record_check_result_inner(scope, phase, params, false)
            .await
    }

    pub async fn record_check_failure(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: RecordCheckResultParams,
    ) -> MethodologyResult<AckResponse> {
        self.record_check_result_inner(scope, phase, params, true)
            .await
    }

    async fn record_check_result_inner(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: RecordCheckResultParams,
        force_failure_event: bool,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::CheckRecord, phase)?;
        if let Some(task_id) = params.scope.task_id() {
            let task_spec_id = self.resolve_spec_for_task(task_id).await?;
            if task_spec_id != params.spec_id {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/scope/task_id".into(),
                    expected: format!("task in spec {}", params.spec_id),
                    actual: format!("task in spec {task_spec_id}"),
                    remediation: "record task-scoped checks against tasks in the same spec".into(),
                });
            }
        }
        self.ensure_findings_exist(params.spec_id, &params.finding_ids)
            .await?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        let tool_name = if force_failure_event {
            "record_check_failure"
        } else {
            "record_check_result"
        };
        self.run_idempotent_mutation(
            tool_name,
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let result = CheckResult {
                    run_id: params.check_run_id,
                    spec_id: params.spec_id,
                    kind: params.kind,
                    scope: params.scope,
                    status: params.status,
                    summary: super::errors::require_non_empty(
                        "/summary",
                        &params.summary,
                        Some(500),
                    )?,
                    evidence_refs: params.evidence_refs,
                    finding_ids: params.finding_ids,
                    recorded_at: Utc::now(),
                };
                let event = if force_failure_event || matches!(result.status, CheckStatus::Fail) {
                    MethodologyEvent::CheckFailureRecorded(CheckFailureRecorded {
                        result,
                        idempotency_key: params.idempotency_key,
                    })
                } else {
                    MethodologyEvent::CheckResultRecorded(CheckResultRecorded {
                        result,
                        idempotency_key: params.idempotency_key,
                    })
                };
                self.emit(phase, event).await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }
}
