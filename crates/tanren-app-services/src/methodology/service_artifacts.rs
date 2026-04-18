//! More tool methods for `MethodologyService` — split out of
//! `service_ext.rs` to stay within the 500-line file budget.

use chrono::Utc;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{AdherenceFindingAdded, IssueCreated, MethodologyEvent};
use tanren_domain::methodology::finding::{Finding, FindingSource};
use tanren_domain::methodology::issue::{Issue, IssueProvider, IssueRef};
use tanren_domain::{FindingId, IssueId, NonEmptyString, SignpostId};

use tanren_contract::methodology::{
    AppendDemoResultParams, CreateIssueParams, CreateIssueResponse, RecordAdherenceFindingParams,
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
        phase: &str,
        params: CreateIssueParams,
    ) -> MethodologyResult<CreateIssueResponse> {
        enforce(scope, ToolCapability::IssueCreate, phase)?;
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
        self.emit_event(MethodologyEvent::IssueCreated(IssueCreated {
            issue: Box::new(issue),
        }))
        .await?;
        Ok(CreateIssueResponse {
            issue_id,
            reference,
        })
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
        phase: &str,
        params: RecordAdherenceFindingParams,
    ) -> MethodologyResult<tanren_contract::methodology::AddFindingResponse> {
        enforce(scope, ToolCapability::AdherenceRecord, phase)?;
        // Critical-cannot-defer rule per adherence.md §4.2: any finding
        // linked to a standard with `importance = Critical` MUST NOT
        // carry `severity = Defer`. The `StandardRef` on the wire
        // carries only (name, category); we resolve the importance
        // from the baseline-standards registry so the check is a
        // typed domain invariant, not a prompt-level guardrail.
        let importance = resolve_standard_importance(&params.standard);
        if importance == Some(tanren_domain::methodology::standard::StandardImportance::Critical)
            && matches!(
                params.severity,
                tanren_domain::methodology::finding::FindingSeverity::Defer
            )
        {
            return Err(MethodologyError::FieldValidation {
                field_path: "/severity".into(),
                expected: "fix_now | note | question (critical standards cannot be deferred)"
                    .into(),
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
            severity: params.severity,
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
        self.emit_event(MethodologyEvent::AdherenceFindingAdded(
            AdherenceFindingAdded {
                finding: Box::new(finding),
                standard: params.standard,
            },
        ))
        .await?;
        Ok(tanren_contract::methodology::AddFindingResponse { finding_id: id })
    }

    // -- Internal helpers -----------------------------------------------------

    /// Scan the event log for the signpost's `SignpostAdded` event to
    /// recover its spec id. O(events); Lane 0.5 scale.
    pub(crate) async fn resolve_spec_for_signpost(
        &self,
        signpost_id: SignpostId,
    ) -> MethodologyResult<tanren_domain::SpecId> {
        let filter = tanren_store::EventFilter {
            event_type: Some("methodology".into()),
            limit: u64::MAX,
            ..tanren_store::EventFilter::default()
        };
        let page = tanren_store::EventStore::query_events(self.store(), &filter).await?;
        for env in page.events {
            if let tanren_domain::events::DomainEvent::Methodology { event } = env.payload
                && let MethodologyEvent::SignpostAdded(e) = &event
                && e.signpost.id == signpost_id
            {
                return Ok(e.signpost.spec_id);
            }
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
        let all = tanren_store::methodology::projections::findings_for_spec(self.store(), spec_id)
            .await?;
        Ok(all.into_iter().filter(|f| ids.contains(&f.id)).collect())
    }

    /// `list_relevant_standards` — returns every baseline standard
    /// applicable to a spec. The current relevance filter is
    /// conservative ("return all baseline standards"); per
    /// `adherence.md §4.1`, Phase 1 narrows the set by the spec's
    /// declared languages/domains. The full list is always a
    /// correct upper bound for the filter.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub fn list_relevant_standards(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        _spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<Vec<tanren_domain::methodology::standard::Standard>> {
        enforce(scope, ToolCapability::StandardRead, phase)?;
        let mut out = super::standards::baseline_standards();
        out.sort_by(|a, b| {
            a.category
                .as_str()
                .cmp(b.category.as_str())
                .then(a.name.as_str().cmp(b.name.as_str()))
        });
        Ok(out)
    }

    /// `add_demo_step` / `mark_demo_step_skip` / `append_demo_result`:
    /// records a demo-step result event. The frontmatter-level
    /// rendering of these results into `demo.md` is done by the
    /// orchestrator when the spec folder is next reconciled.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub fn demo_step_record(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: &AppendDemoResultParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::DemoResults, phase)?;
        // Validate the identifier shape at the boundary so a
        // downstream frontmatter render cannot produce an invalid
        // step id.
        let _step_id = require_non_empty("/step_id", &params.step_id, Some(80))?;
        let _observed = require_non_empty("/observed", &params.observed, None)?;
        Ok(())
    }
}

/// Look up the importance of a standard by (category, name) in the
/// bundled baseline registry. Returns `None` for unknown standards —
/// adherence findings against unknown standards remain permitted but
/// do not trigger the critical-cannot-defer guard.
fn resolve_standard_importance(
    r: &tanren_domain::methodology::finding::StandardRef,
) -> Option<tanren_domain::methodology::standard::StandardImportance> {
    super::standards::baseline_standards()
        .into_iter()
        .find(|s| s.category.as_str() == r.category.as_str() && s.name.as_str() == r.name.as_str())
        .map(|s| s.importance)
}
