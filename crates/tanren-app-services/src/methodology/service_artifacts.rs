//! More tool methods for `MethodologyService` — split out of
//! `service_ext.rs` to stay within the 500-line file budget.

use chrono::Utc;
use tanren_domain::entity::EntityRef;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{AdherenceFindingAdded, IssueCreated, MethodologyEvent};
use tanren_domain::methodology::finding::{AdherenceSeverity, Finding, FindingSource};
use tanren_domain::methodology::issue::{Issue, IssueProvider, IssueRef};
use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
use tanren_domain::methodology::validation::validate_finding_line_numbers;
use tanren_domain::{FindingId, IssueId, NonEmptyString, SignpostId};

use tanren_contract::methodology::{
    CreateIssueParams, CreateIssueResponse, RecordAdherenceFindingParams, SchemaVersion,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};
use super::service::MethodologyService;

impl MethodologyService {
    // -- §3.7 create_issue ----------------------------------------------------

    /// `create_issue` — records a backlog item for `triage-audits` or
    /// `handle-feedback`.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn create_issue(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: CreateIssueParams,
    ) -> MethodologyResult<CreateIssueResponse> {
        enforce(scope, ToolCapability::IssueCreate, phase)?;
        require_phase_in(
            "create_issue",
            phase,
            &[KnownPhase::TriageAudits, KnownPhase::HandleFeedback],
        )?;
        let spec_id = params.origin_spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "create_issue",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                let title = require_non_empty("/title", &params.title, Some(200))?;
                let scope_label = require_non_empty(
                    "/suggested_spec_scope",
                    &params.suggested_spec_scope,
                    Some(120),
                )?;
                // Issues are recorded as stable URNs at creation time. The
                // orchestrator's provider adapter later reconciles the URN to
                // the tracker-assigned URL (GitHub issue number, etc.) by
                // folding subsequent `IssueCreated` events into its outbox.
                // No placeholder URL — the URN IS the canonical reference
                // until reconciled.
                let issue_id = IssueId::new();
                let urn = format!("urn:tanren:issue:{}:{}", params.origin_spec_id, issue_id);
                let reference = IssueRef {
                    provider: IssueProvider::GitHub,
                    number: 0,
                    url: NonEmptyString::try_new(urn)
                        .map_err(|e| MethodologyError::Internal(e.to_string()))?,
                };
                let issue = Issue {
                    id: issue_id,
                    origin_spec_id: params.origin_spec_id,
                    title,
                    description: params.description,
                    suggested_spec_scope: scope_label,
                    priority: params.priority,
                    reference: reference.clone(),
                    created_at: Utc::now(),
                };
                self.emit_event(
                    phase,
                    MethodologyEvent::IssueCreated(IssueCreated {
                        issue: Box::new(issue),
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(CreateIssueResponse {
                    schema_version: SchemaVersion::current(),
                    issue_id,
                    reference,
                })
            },
        )
        .await
    }

    // -- §3.8 adherence + standards read --------------------------------------

    /// `record_adherence_finding` — enforces the
    /// "critical standards cannot defer" rule at the boundary.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn record_adherence_finding(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: RecordAdherenceFindingParams,
    ) -> MethodologyResult<tanren_contract::methodology::AddFindingResponse> {
        enforce(scope, ToolCapability::AdherenceRecord, phase)?;
        require_phase_in(
            "record_adherence_finding",
            phase,
            &[KnownPhase::AdhereTask, KnownPhase::AdhereSpec],
        )?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "record_adherence_finding",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                validate_finding_line_numbers(&params.line_numbers)
                    .map_err(MethodologyError::from)?;
                // Critical-cannot-defer rule per adherence.md §4.2: any finding
                // linked to a standard with `importance = Critical` MUST NOT
                // carry `severity = Defer`. The `StandardRef` on the wire
                // carries only (name, category); we resolve the importance
                // from the baseline-standards registry so the check is a
                // typed domain invariant, not a prompt-level guardrail.
                let standard = resolve_standard(self.standards(), &params.standard).ok_or_else(|| {
                    MethodologyError::FieldValidation {
                        field_path: "/standard".into(),
                        expected: "existing standard (category + name) from runtime standards registry".into(),
                        actual: format!(
                            "{}:{}",
                            params.standard.category.as_str(),
                            params.standard.name.as_str()
                        ),
                        remediation: "list relevant standards for this spec and choose one of the returned standards".into(),
                    }
                })?;
                if standard.importance.disallows_defer()
                    && matches!(params.severity, AdherenceSeverity::Defer)
                {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/severity".into(),
                        expected: "fix_now (critical standards cannot be deferred)".into(),
                        actual: "defer".into(),
                        remediation: format!(
                            "raise severity to `fix_now` or reclassify `{}:{}` as non-critical",
                            params.standard.category.as_str(),
                            params.standard.name.as_str()
                        ),
                    });
                }
                let title = NonEmptyString::try_new(format!(
                    "adherence:{}:{}",
                    params.standard.category.as_str(),
                    params.standard.name.as_str()
                ))
                .map_err(|e| MethodologyError::Internal(e.to_string()))?;
                let finding = Finding {
                    id: FindingId::new(),
                    spec_id: params.spec_id,
                    severity: params.severity.as_finding_severity(),
                    title,
                    description: params.rationale,
                    affected_files: params.affected_files,
                    line_numbers: params.line_numbers,
                    source: FindingSource::Adherence {
                        standard: params.standard.clone(),
                    },
                    attached_task: None,
                    created_at: Utc::now(),
                };
                let id = finding.id;
                self.emit_event(
                    phase,
                    MethodologyEvent::AdherenceFindingAdded(AdherenceFindingAdded {
                        finding: Box::new(finding),
                        standard: params.standard,
                        idempotency_key: params.idempotency_key,
                    }),
                )
                .await?;
                Ok(tanren_contract::methodology::AddFindingResponse {
                    schema_version: SchemaVersion::current(),
                    finding_id: id,
                })
            },
        )
        .await
    }

    // -- Internal helpers -----------------------------------------------------

    /// Resolve signpost root to spec id through projection lookup,
    /// with an event-log scan fallback for migration backfill.
    pub(crate) async fn resolve_spec_for_signpost(
        &self,
        signpost_id: SignpostId,
    ) -> MethodologyResult<tanren_domain::SpecId> {
        if let Some(spec_id) = self
            .store()
            .load_methodology_signpost_spec_projection(signpost_id)
            .await?
        {
            return Ok(spec_id);
        }
        tracing::warn!(signpost_id = %signpost_id, "signpost->spec projection miss; attempting targeted recovery");
        let recovered = self
            .first_methodology_event_for_entity(EntityRef::Signpost(signpost_id))
            .await?;
        if let Some(MethodologyEvent::SignpostAdded(e)) = recovered {
            self.store()
                .upsert_methodology_signpost_spec_projection(signpost_id, e.signpost.spec_id)
                .await?;
            return Ok(e.signpost.spec_id);
        }
        Err(MethodologyError::NotFound {
            resource: "signpost".into(),
            key: signpost_id.to_string(),
        })
    }

    /// Load findings referenced by ids and confirm they belong to the
    /// spec. Used by `record_rubric_score` to enforce the `fix_now`
    /// sub-invariant.
    pub(crate) async fn load_findings(
        &self,
        ids: &[FindingId],
        spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<Vec<Finding>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut deduped = std::collections::HashSet::new();
        for id in ids {
            if !deduped.insert(*id) {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/supporting_finding_ids".into(),
                    expected: "unique finding ids".into(),
                    actual: "duplicate id present".into(),
                    remediation: "remove duplicate finding ids from supporting_finding_ids".into(),
                });
            }
        }
        let fetched =
            tanren_store::methodology::projections::findings_by_ids(self.store(), spec_id, ids)
                .await?;
        let by_id: std::collections::HashMap<FindingId, Finding> =
            fetched.into_iter().map(|f| (f.id, f)).collect();
        let mut missing = Vec::new();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            match by_id.get(id) {
                Some(found) => out.push(found.clone()),
                None => missing.push(id.to_string()),
            }
        }
        if !missing.is_empty() {
            return Err(MethodologyError::FieldValidation {
                field_path: "/supporting_finding_ids".into(),
                expected: "all referenced finding ids must exist for spec".into(),
                actual: format!("missing ids: {}", missing.join(", ")),
                remediation: "record findings first, then reference their finding_id values".into(),
            });
        }
        Ok(out)
    }
}

fn require_phase_in(
    tool_name: &str,
    phase: &PhaseId,
    allowed: &[KnownPhase],
) -> MethodologyResult<()> {
    if allowed.iter().any(|known| phase.is_known(*known)) {
        return Ok(());
    }
    let allowed_tags: Vec<&str> = allowed.iter().map(|v| v.tag()).collect();
    Err(MethodologyError::FieldValidation {
        field_path: "/phase".into(),
        expected: format!(
            "{tool_name} allowed only in phases: {}",
            allowed_tags.join(", ")
        ),
        actual: phase.as_str().to_owned(),
        remediation: format!(
            "invoke `{tool_name}` from one of: {}",
            allowed_tags.join(", ")
        ),
    })
}

/// Look up a standard by `(category, name)` in the runtime registry.
fn resolve_standard<'a>(
    standards: &'a [tanren_domain::methodology::standard::Standard],
    r: &tanren_domain::methodology::finding::StandardRef,
) -> Option<&'a tanren_domain::methodology::standard::Standard> {
    standards
        .iter()
        .find(|s| s.category.as_str() == r.category.as_str() && s.name.as_str() == r.name.as_str())
}
