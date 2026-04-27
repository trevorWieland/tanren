use chrono::Utc;
use tanren_contract::methodology::{
    AckResponse, AddFindingParams, AddFindingResponse, FindingLifecycleParams, FindingScopeFilter,
    ListFindingsParams, ListFindingsResponse, SchemaVersion, SupersedeFindingParams,
};
use tanren_domain::FindingId;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::check::CheckKind;
use tanren_domain::methodology::events::{
    FindingAdded, FindingDeferred, FindingReopened, FindingResolved, FindingStillOpen,
    FindingSuperseded, MethodologyEvent,
};
use tanren_domain::methodology::finding::{Finding, FindingSource, FindingView};
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

    /// `list_findings` — read-only projected finding lifecycle view.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn list_findings(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: ListFindingsParams,
    ) -> MethodologyResult<ListFindingsResponse> {
        enforce(scope, ToolCapability::FindingRead, phase)?;
        let mut findings =
            tanren_store::methodology::finding_views_for_spec(self.store(), params.spec_id).await?;
        findings.retain(|view| finding_matches_filters(view, &params));
        Ok(ListFindingsResponse {
            schema_version: SchemaVersion::current(),
            findings,
        })
    }

    /// `resolve_finding`.
    pub async fn resolve_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: FindingLifecycleParams,
    ) -> MethodologyResult<AckResponse> {
        self.record_finding_lifecycle(scope, phase, "resolve_finding", params, |p, phase| {
            MethodologyEvent::FindingResolved(FindingResolved {
                finding_id: p.finding_id,
                spec_id: p.spec_id,
                evidence: p.evidence,
                source_phase: phase.clone(),
                idempotency_key: p.idempotency_key,
            })
        })
        .await
    }

    /// `reopen_finding`.
    pub async fn reopen_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: FindingLifecycleParams,
    ) -> MethodologyResult<AckResponse> {
        self.record_finding_lifecycle(scope, phase, "reopen_finding", params, |p, phase| {
            MethodologyEvent::FindingReopened(FindingReopened {
                finding_id: p.finding_id,
                spec_id: p.spec_id,
                evidence: p.evidence,
                source_phase: phase.clone(),
                idempotency_key: p.idempotency_key,
            })
        })
        .await
    }

    /// `defer_finding`.
    pub async fn defer_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: FindingLifecycleParams,
    ) -> MethodologyResult<AckResponse> {
        self.record_finding_lifecycle(scope, phase, "defer_finding", params, |p, phase| {
            MethodologyEvent::FindingDeferred(FindingDeferred {
                finding_id: p.finding_id,
                spec_id: p.spec_id,
                evidence: p.evidence,
                source_phase: phase.clone(),
                idempotency_key: p.idempotency_key,
            })
        })
        .await
    }

    /// `record_finding_still_open`.
    pub async fn record_finding_still_open(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: FindingLifecycleParams,
    ) -> MethodologyResult<AckResponse> {
        self.record_finding_lifecycle(
            scope,
            phase,
            "record_finding_still_open",
            params,
            |p, phase| {
                MethodologyEvent::FindingStillOpen(FindingStillOpen {
                    finding_id: p.finding_id,
                    spec_id: p.spec_id,
                    evidence: p.evidence,
                    source_phase: phase.clone(),
                    idempotency_key: p.idempotency_key,
                })
            },
        )
        .await
    }

    /// `supersede_finding`.
    pub async fn supersede_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: SupersedeFindingParams,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::FindingLifecycle, phase)?;
        if params.superseded_by.is_empty() {
            return Err(MethodologyError::FieldValidation {
                field_path: "/superseded_by".into(),
                expected: "at least one replacement finding id".into(),
                actual: "[]".into(),
                remediation: "provide replacement finding ids before superseding".into(),
            });
        }
        let ids = std::iter::once(params.finding_id)
            .chain(params.superseded_by.iter().copied())
            .collect::<Vec<_>>();
        self.ensure_findings_exist(params.spec_id, &ids).await?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "supersede_finding",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                self.emit(
                    phase,
                    MethodologyEvent::FindingSuperseded(FindingSuperseded {
                        finding_id: params.finding_id,
                        spec_id: params.spec_id,
                        superseded_by: params.superseded_by,
                        evidence: params.evidence,
                        source_phase: phase.clone(),
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    async fn record_finding_lifecycle<F>(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        tool_name: &'static str,
        params: FindingLifecycleParams,
        make_event: F,
    ) -> MethodologyResult<AckResponse>
    where
        F: FnOnce(FindingLifecycleParams, &PhaseId) -> MethodologyEvent,
    {
        enforce(scope, ToolCapability::FindingLifecycle, phase)?;
        self.ensure_findings_exist(params.spec_id, &[params.finding_id])
            .await?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            tool_name,
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let event = make_event(params, phase);
                self.emit(phase, event).await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    pub(crate) async fn ensure_findings_exist(
        &self,
        spec_id: tanren_domain::SpecId,
        ids: &[FindingId],
    ) -> MethodologyResult<Vec<FindingView>> {
        let views =
            tanren_store::methodology::finding_views_by_ids(self.store(), spec_id, ids).await?;
        let by_id: std::collections::HashMap<FindingId, FindingView> = views
            .into_iter()
            .map(|view| (view.finding.id, view))
            .collect();
        let mut missing = Vec::new();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            match by_id.get(id) {
                Some(view) => out.push(view.clone()),
                None => missing.push(id.to_string()),
            }
        }
        if !missing.is_empty() {
            return Err(MethodologyError::FieldValidation {
                field_path: "/finding_id".into(),
                expected: "finding ids that exist in this spec".into(),
                actual: format!("missing ids: {}", missing.join(", ")),
                remediation: "record findings before lifecycle transitions".into(),
            });
        }
        Ok(out)
    }
}

fn finding_matches_filters(view: &FindingView, params: &ListFindingsParams) -> bool {
    if params.status.is_some_and(|status| view.status != status) {
        return false;
    }
    if params
        .severity
        .is_some_and(|severity| view.finding.severity != severity)
    {
        return false;
    }
    if params
        .task_id
        .is_some_and(|task_id| view.finding.attached_task != Some(task_id))
    {
        return false;
    }
    if params.scope.is_some_and(|scope| match scope {
        FindingScopeFilter::Spec => view.finding.attached_task.is_some(),
        FindingScopeFilter::Task => view.finding.attached_task.is_none(),
    }) {
        return false;
    }
    if params.check_kind.as_ref().is_some_and(|check_kind| {
        let source_matches = matches!(
            (&view.finding.source, check_kind),
            (FindingSource::Audit { .. }, CheckKind::Audit)
                | (FindingSource::Adherence { .. }, CheckKind::Adherence,)
                | (FindingSource::Demo { .. }, CheckKind::Demo)
        );
        let lifecycle_matches = view
            .lifecycle_evidence
            .as_ref()
            .and_then(|evidence| evidence.check_kind.as_ref())
            == Some(check_kind);
        !source_matches && !lifecycle_matches
    }) {
        return false;
    }
    if params.source_phase.as_ref().is_some_and(|phase| {
        !matches!(
            &view.finding.source,
            FindingSource::Audit { phase: source, .. } if source == phase
        )
    }) {
        return false;
    }
    true
}
