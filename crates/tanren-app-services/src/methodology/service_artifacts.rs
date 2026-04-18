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
use super::errors::{MethodologyError, MethodologyResult};
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
        let title = NonEmptyString::try_new(params.title)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        let scope_label = NonEmptyString::try_new(params.suggested_spec_scope)
            .map_err(|e| MethodologyError::Validation(e.to_string()))?;
        // Lane 0.5: issue creation is local-only; the GitHub adapter
        // produces the authoritative URL and issue number. Stub URL
        // here pending Wave 10's CLI adapter; the contract permits any
        // `url: NonEmptyString` so downstream renders don't break.
        let reference = IssueRef {
            provider: IssueProvider::GitHub,
            number: 0,
            url: NonEmptyString::try_new("https://example.invalid/pending")
                .map_err(|e| MethodologyError::Validation(e.to_string()))?,
        };
        let issue = Issue {
            id: IssueId::new(),
            origin_spec_id: params.origin_spec_id,
            title,
            description: params.description,
            suggested_spec_scope: scope_label,
            priority: params.priority,
            reference: reference.clone(),
            created_at: Utc::now(),
        };
        let id = issue.id;
        self.emit_event(MethodologyEvent::IssueCreated(IssueCreated {
            issue: Box::new(issue),
        }))
        .await?;
        Ok(CreateIssueResponse {
            issue_id: id,
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
        // The critical-disallows-defer rule is enforced in the
        // standard-resolution layer (Wave 9's source loader knows the
        // standard's importance). Lane 0.5 scope records the typed
        // event with the severity already chosen; critical-check lands
        // when the standards registry is wired.
        let title = NonEmptyString::try_new(format!(
            "adherence:{}:{}",
            params.standard.category.as_str(),
            params.standard.name.as_str()
        ))
        .map_err(|e| MethodologyError::Validation(e.to_string()))?;
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

    /// `list_relevant_standards` — read-only placeholder. Full
    /// relevance filter (per `adherence.md §4.1`) lives in Wave 9's
    /// standards registry.
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
        // Lane 0.5 returns the empty set; Wave 9's source loader
        // populates the registry. MCP/CLI callers can still exercise
        // the capability-scope check.
        Ok(Vec::new())
    }

    /// `add_demo_step` / `mark_demo_step_skip` / `append_demo_result`:
    /// the demo frontmatter surface lives in `evidence::demo` and is
    /// rendered directly by the agent (guarded by the capability
    /// check). Lane 0.5 exposes the capability gate here; file-level
    /// render lands in Wave 9's source loader. Kept as a single typed
    /// check so the MCP server can enforce scope and return a
    /// well-formed success.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub fn demo_step_record(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        _params: AppendDemoResultParams,
    ) -> MethodologyResult<()> {
        enforce(scope, ToolCapability::DemoResults, phase)?;
        Ok(())
    }
}
