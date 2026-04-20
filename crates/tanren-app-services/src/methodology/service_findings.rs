use chrono::Utc;
use tanren_contract::methodology::{AddFindingParams, AddFindingResponse, SchemaVersion};
use tanren_domain::FindingId;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{FindingAdded, MethodologyEvent};
use tanren_domain::methodology::finding::Finding;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::validation::{
    validate_finding_attached_task_spec, validate_finding_line_numbers,
};

use super::MethodologyService;
use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};

impl MethodologyService {
    // -- §3.2 Findings --------------------------------------------------------

    /// `add_finding` — emit [`MethodologyEvent::FindingAdded`].
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn add_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: AddFindingParams,
    ) -> MethodologyResult<AddFindingResponse> {
        enforce(scope, ToolCapability::FindingAdd, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "add_finding",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let title = require_non_empty("/title", &params.title, Some(200))?;
                validate_finding_line_numbers(&params.line_numbers)
                    .map_err(MethodologyError::from)?;
                if let Some(attached_task) = params.attached_task {
                    let task_spec_id = self.resolve_spec_for_task(attached_task).await?;
                    validate_finding_attached_task_spec(
                        attached_task,
                        params.spec_id,
                        task_spec_id,
                    )
                    .map_err(MethodologyError::from)?;
                }
                let finding = Finding {
                    id: FindingId::new(),
                    spec_id: params.spec_id,
                    severity: params.severity,
                    title,
                    description: params.description,
                    affected_files: params.affected_files,
                    line_numbers: params.line_numbers,
                    source: params.source,
                    attached_task: params.attached_task,
                    created_at: Utc::now(),
                };
                let id = finding.id;
                self.emit(
                    phase,
                    MethodologyEvent::FindingAdded(FindingAdded {
                        finding: Box::new(finding),
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(AddFindingResponse {
                    schema_version: SchemaVersion::current(),
                    finding_id: id,
                })
            },
        )
        .await
    }
}
