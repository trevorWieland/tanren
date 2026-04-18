//! More tool methods for `MethodologyService` — split out of
//! `service_ext.rs` to stay within the 500-line file budget.

use chrono::Utc;
use tanren_domain::entity::EntityRef;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{AdherenceFindingAdded, IssueCreated, MethodologyEvent};
use tanren_domain::methodology::finding::{Finding, FindingSource};
use tanren_domain::methodology::issue::{Issue, IssueProvider, IssueRef};
use tanren_domain::{FindingId, IssueId, NonEmptyString, SignpostId};

use tanren_contract::methodology::{
    CreateIssueParams, CreateIssueResponse, RecordAdherenceFindingParams, SchemaVersion,
};

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult, require_non_empty};
use super::service::MethodologyService;

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;

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
        require_phase_in("create_issue", phase, &["triage-audits", "handle-feedback"])?;
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
        phase: &str,
        params: RecordAdherenceFindingParams,
    ) -> MethodologyResult<tanren_contract::methodology::AddFindingResponse> {
        enforce(scope, ToolCapability::AdherenceRecord, phase)?;
        let spec_id = params.spec_id;
        let explicit_key = params.idempotency_key.clone();
        let idempotency_payload = params.clone();
        self.run_idempotent_mutation(
            "record_adherence_finding",
            spec_id,
            explicit_key,
            &idempotency_payload,
            || async move {
                // Critical-cannot-defer rule per adherence.md §4.2: any finding
                // linked to a standard with `importance = Critical` MUST NOT
                // carry `severity = Defer`. The `StandardRef` on the wire
                // carries only (name, category); we resolve the importance
                // from the baseline-standards registry so the check is a
                // typed domain invariant, not a prompt-level guardrail.
                let importance = resolve_standard_importance(self.standards(), &params.standard);
                if importance
                    == Some(tanren_domain::methodology::standard::StandardImportance::Critical)
                    && matches!(
                        params.severity,
                        tanren_domain::methodology::finding::FindingSeverity::Defer
                    )
                {
                    return Err(MethodologyError::FieldValidation {
                        field_path: "/severity".into(),
                        expected:
                            "fix_now | note | question (critical standards cannot be deferred)"
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

    /// Resolve signpost root to spec id through indexed signpost-root
    /// event queries.
    pub(crate) async fn resolve_spec_for_signpost(
        &self,
        signpost_id: SignpostId,
    ) -> MethodologyResult<tanren_domain::SpecId> {
        if let Ok(cache) = self.signpost_spec_cache.lock()
            && let Some(spec_id) = cache.get(&signpost_id)
        {
            return Ok(*spec_id);
        }
        let mut cursor = None;
        loop {
            let filter = tanren_store::EventFilter {
                entity_ref: Some(EntityRef::Signpost(signpost_id)),
                event_type: Some("methodology".into()),
                limit: METHODOLOGY_PAGE_SIZE,
                cursor,
                ..tanren_store::EventFilter::default()
            };
            let page = tanren_store::EventStore::query_events(self.store(), &filter).await?;
            for env in page.events {
                if let tanren_domain::events::DomainEvent::Methodology { event } = env.payload
                    && let MethodologyEvent::SignpostAdded(e) = &event
                {
                    if let Ok(mut cache) = self.signpost_spec_cache.lock() {
                        cache.insert(signpost_id, e.signpost.spec_id);
                    }
                    return Ok(e.signpost.spec_id);
                }
            }
            if !page.has_more {
                break;
            }
            cursor = page.next_cursor;
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

    /// `list_relevant_standards` — baseline-complete upper bound.
    /// Preserved for callers that do not supply relevance filters; new
    /// callers should prefer
    /// [`Self::list_relevant_standards_filtered`] which implements
    /// the adherence §4.1 algorithm.
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
        let mut out = self.standards().to_vec();
        out.sort_by(|a, b| {
            a.category
                .as_str()
                .cmp(b.category.as_str())
                .then(a.name.as_str().cmp(b.name.as_str()))
        });
        Ok(out)
    }

    /// Implements the adherence §4.1 relevance algorithm: for each
    /// baseline standard, keep the standard if (and explain why) any
    /// of:
    ///
    /// - one of the `touched_files` matches one of the standard's
    ///   `applies_to` globs, or
    /// - `project_language` matches one of `applies_to_languages`, or
    /// - one of `domains` matches one of `applies_to_domains`, or
    /// - the standard declares no per-axis filter (fully universal).
    ///
    /// With all filter inputs empty, every baseline standard is
    /// returned — preserving the conservative upper-bound behavior for
    /// pre-Lane-0.5 callers. The `inclusion_reason` field on every
    /// returned `RelevantStandard` names the axis that matched so
    /// operators can audit inclusion decisions.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub fn list_relevant_standards_filtered(
        &self,
        scope: &CapabilityScope,
        phase: &str,
        params: &tanren_contract::methodology::ListRelevantStandardsParams,
    ) -> MethodologyResult<Vec<tanren_contract::methodology::RelevantStandard>> {
        enforce(scope, ToolCapability::StandardRead, phase)?;
        let all = self.standards().to_vec();
        let all_empty = params.touched_files.is_empty()
            && params.project_language.is_none()
            && params.domains.is_empty();

        let mut out: Vec<tanren_contract::methodology::RelevantStandard> = all
            .into_iter()
            .filter_map(|s| {
                if all_empty {
                    return Some(tanren_contract::methodology::RelevantStandard {
                        schema_version: SchemaVersion::current(),
                        standard: s,
                        inclusion_reason: "baseline upper bound (no filter inputs supplied)".into(),
                    });
                }
                relevance_reason(&s, params).map(|reason| {
                    tanren_contract::methodology::RelevantStandard {
                        schema_version: SchemaVersion::current(),
                        standard: s,
                        inclusion_reason: reason,
                    }
                })
            })
            .collect();
        out.sort_by(|a, b| {
            a.standard
                .category
                .as_str()
                .cmp(b.standard.category.as_str())
                .then(a.standard.name.as_str().cmp(b.standard.name.as_str()))
        });
        Ok(out)
    }
}

fn require_phase_in(tool_name: &str, phase: &str, allowed: &[&str]) -> MethodologyResult<()> {
    if allowed.contains(&phase) {
        return Ok(());
    }
    Err(MethodologyError::FieldValidation {
        field_path: "/phase".into(),
        expected: format!("{tool_name} allowed only in phases: {}", allowed.join(", ")),
        actual: phase.to_owned(),
        remediation: format!("invoke `{tool_name}` from one of: {}", allowed.join(", ")),
    })
}

/// Evaluate the per-axis relevance filter. Returns
/// `Some(explanation)` when the standard should be included,
/// `None` when every axis excludes it.
fn relevance_reason(
    standard: &tanren_domain::methodology::standard::Standard,
    params: &tanren_contract::methodology::ListRelevantStandardsParams,
) -> Option<String> {
    // A standard with zero `applies_to*` entries declares itself as
    // universally-applicable. Keep it unless the caller explicitly
    // scoped to a language/domain this standard does not claim.
    let is_universal = standard.applies_to.is_empty()
        && standard.applies_to_languages.is_empty()
        && standard.applies_to_domains.is_empty();
    if is_universal {
        return Some("universal (no per-axis restriction declared)".into());
    }
    if !standard.applies_to.is_empty()
        && let Some(file) = matching_touched_file(standard, &params.touched_files)
    {
        return Some(format!(
            "matched `applies_to` against touched file `{file}`"
        ));
    }
    if let Some(lang) = params.project_language.as_deref()
        && !standard.applies_to_languages.is_empty()
        && standard
            .applies_to_languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(lang))
    {
        return Some(format!("matched `applies_to_languages` against `{lang}`"));
    }
    if !params.domains.is_empty()
        && !standard.applies_to_domains.is_empty()
        && let Some(d) = params
            .domains
            .iter()
            .find(|d| standard.applies_to_domains.iter().any(|sd| sd == *d))
    {
        return Some(format!("matched `applies_to_domains` against `{d}`"));
    }
    None
}

/// Return the first caller-supplied touched file that matches any of
/// the standard's `applies_to` globs. Uses a lightweight suffix /
/// pattern match (no full-glob engine required for baseline entries
/// like `**/*.rs`, `*.py`, `src/**`).
fn matching_touched_file<'a>(
    standard: &tanren_domain::methodology::standard::Standard,
    touched: &'a [String],
) -> Option<&'a str> {
    touched
        .iter()
        .find(|f| {
            standard
                .applies_to
                .iter()
                .any(|pat| simple_glob_match(pat, f))
        })
        .map(String::as_str)
}

/// Lightweight glob matcher covering the baseline patterns used by
/// built-in standards: `**/*.ext`, `*.ext`, `prefix/**`. Exact-string
/// patterns always fall back to equality. This is intentionally
/// simple — adding a full glob crate (e.g. `globset`) is a Phase-1
/// concern once downstream consumers need richer patterns.
fn simple_glob_match(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }
    if let Some(ext) = pattern.strip_prefix("**/*") {
        return path.ends_with(ext);
    }
    if let Some(ext) = pattern.strip_prefix("*") {
        return path.ends_with(ext);
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix) && path.len() > prefix.len();
    }
    false
}

/// Look up the importance of a standard by (category, name) in the
/// bundled baseline registry. Returns `None` for unknown standards —
/// adherence findings against unknown standards remain permitted but
/// do not trigger the critical-cannot-defer guard.
fn resolve_standard_importance(
    standards: &[tanren_domain::methodology::standard::Standard],
    r: &tanren_domain::methodology::finding::StandardRef,
) -> Option<tanren_domain::methodology::standard::StandardImportance> {
    standards
        .iter()
        .find(|s| s.category.as_str() == r.category.as_str() && s.name.as_str() == r.name.as_str())
        .map(|s| s.importance)
}
