//! Standards read/relevance methods for [`MethodologyService`].
//!
//! Split from `service_artifacts.rs` to satisfy the 500-line file
//! budget while keeping related relevance logic together.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};
use tanren_contract::methodology::{
    ListRelevantStandardsParams, ListRelevantStandardsResponse, SchemaVersion,
};
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::events::{MethodologyEvent, SpecFrontmatterPatch};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::spec::SpecRelevanceContext;

use super::capabilities::enforce;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;

const METHODOLOGY_PAGE_SIZE: u64 = 1_000;
static GLOBSET_CACHE: OnceLock<Mutex<HashMap<String, Result<GlobSet, String>>>> = OnceLock::new();

impl MethodologyService {
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
        phase: &PhaseId,
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
    /// returned — preserving the conservative upper-bound behavior. The
    /// `inclusion_reason` field on every
    /// returned `RelevantStandard` names the axis that matched so
    /// operators can audit inclusion decisions.
    ///
    /// # Errors
    /// See [`MethodologyError`].
    pub async fn list_relevant_standards_filtered(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: &ListRelevantStandardsParams,
    ) -> MethodologyResult<ListRelevantStandardsResponse> {
        enforce(scope, ToolCapability::StandardRead, phase)?;
        let derived = self.derive_relevance_context(params.spec_id).await?;
        let effective = EffectiveRelevanceInputs::from_derived_and_hints(&derived, params);
        let all = self.standards().to_vec();
        let all_empty = effective.touched_files.is_empty()
            && effective.project_languages.is_empty()
            && effective.domains.is_empty();

        let mut out: Vec<tanren_contract::methodology::RelevantStandard> = Vec::new();
        for standard in all {
            if all_empty {
                out.push(tanren_contract::methodology::RelevantStandard {
                    schema_version: SchemaVersion::current(),
                    standard,
                    inclusion_reason: "baseline upper bound (no filter inputs supplied)".into(),
                });
                continue;
            }
            if let Some(inclusion_reason) = relevance_reason(&standard, &effective)? {
                out.push(tanren_contract::methodology::RelevantStandard {
                    schema_version: SchemaVersion::current(),
                    standard,
                    inclusion_reason,
                });
            }
        }
        out.sort_by(|a, b| {
            a.standard
                .category
                .as_str()
                .cmp(b.standard.category.as_str())
                .then(a.standard.name.as_str().cmp(b.standard.name.as_str()))
        });
        Ok(ListRelevantStandardsResponse {
            schema_version: SchemaVersion::current(),
            standards: out,
        })
    }

    /// Param-struct wrapper so transports can dispatch from a single
    /// compile-time registry.
    pub async fn list_relevant_standards_from_params(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: ListRelevantStandardsParams,
    ) -> MethodologyResult<ListRelevantStandardsResponse> {
        self.list_relevant_standards_filtered(scope, phase, &params)
            .await
    }

    async fn derive_relevance_context(
        &self,
        spec_id: tanren_domain::SpecId,
    ) -> MethodologyResult<SpecRelevanceContext> {
        let events = tanren_store::methodology::projections::load_methodology_events(
            self.store(),
            spec_id,
            METHODOLOGY_PAGE_SIZE,
        )
        .await?;
        let mut relevance_context = SpecRelevanceContext::default();
        for event in events {
            match event {
                MethodologyEvent::SpecDefined(e) => {
                    relevance_context = e.spec.relevance_context.clone();
                }
                MethodologyEvent::SpecFrontmatterUpdated(e) => {
                    if let SpecFrontmatterPatch::SetRelevanceContext {
                        relevance_context: next,
                    } = e.patch
                    {
                        relevance_context = next;
                    }
                }
                _ => {}
            }
        }
        Ok(relevance_context)
    }
}

/// Evaluate the per-axis relevance filter. Returns
/// `Some(explanation)` when the standard should be included,
/// `None` when every axis excludes it.
fn relevance_reason(
    standard: &tanren_domain::methodology::standard::Standard,
    params: &EffectiveRelevanceInputs,
) -> MethodologyResult<Option<String>> {
    // A standard with zero `applies_to*` entries declares itself as
    // universally-applicable. Keep it unless the caller explicitly
    // scoped to a language/domain this standard does not claim.
    let is_universal = standard.applies_to.is_empty()
        && standard.applies_to_languages.is_empty()
        && standard.applies_to_domains.is_empty();
    if is_universal {
        return Ok(Some("universal (no per-axis restriction declared)".into()));
    }
    if !standard.applies_to.is_empty()
        && let Some(file) = matching_touched_file(standard, &params.touched_files)?
    {
        return Ok(Some(format!(
            "matched `applies_to` against touched file `{file}`"
        )));
    }
    if !params.project_languages.is_empty() && !standard.applies_to_languages.is_empty() {
        let std_langs: std::collections::HashSet<String> = standard
            .applies_to_languages
            .iter()
            .map(|v| normalize_match_label(v))
            .filter(|v| !v.is_empty())
            .collect();
        if let Some(lang) = params
            .project_languages
            .iter()
            .find(|lang| std_langs.contains(*lang))
        {
            return Ok(Some(format!(
                "matched `applies_to_languages` against `{lang}`"
            )));
        }
    }
    if !params.domains.is_empty() && !standard.applies_to_domains.is_empty() {
        let std_domains: std::collections::HashSet<String> = standard
            .applies_to_domains
            .iter()
            .map(|v| normalize_match_label(v))
            .filter(|v| !v.is_empty())
            .collect();
        if let Some(domain) = params
            .domains
            .iter()
            .find(|domain| std_domains.contains(*domain))
        {
            return Ok(Some(format!(
                "matched `applies_to_domains` against `{domain}`"
            )));
        }
    }
    Ok(None)
}

/// Return the first touched file that matches any of the standard's
/// `applies_to` globs using `globset` semantics.
fn matching_touched_file<'a>(
    standard: &tanren_domain::methodology::standard::Standard,
    touched: &'a [String],
) -> MethodologyResult<Option<&'a str>> {
    if touched.is_empty() || standard.applies_to.is_empty() {
        return Ok(None);
    }
    let cache_key = matcher_cache_key(standard);
    let cache = GLOBSET_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = cache
        .lock()
        .map_err(|_| MethodologyError::Internal("globset cache lock poisoned".into()))?;
    let Some(cached) = guard.get(&cache_key) else {
        drop(guard);
        let mut guard = cache
            .lock()
            .map_err(|_| MethodologyError::Internal("globset cache lock poisoned".into()))?;
        guard.insert(
            cache_key.clone(),
            build_globset(standard).map_err(|err| err.to_string()),
        );
        drop(guard);
        return matching_touched_file(standard, touched);
    };
    let globset = match cached {
        Ok(globset) => globset,
        Err(reason) => {
            return Err(MethodologyError::FieldValidation {
                field_path: format!(
                    "/standards/{}/{}/applies_to",
                    standard.category.as_str(),
                    standard.name.as_str()
                ),
                expected: "valid glob patterns".into(),
                actual: format!("{:?}", standard.applies_to),
                remediation: reason.clone(),
            });
        }
    };
    Ok(touched
        .iter()
        .find(|f| globset.is_match(normalize_path(f)))
        .map(String::as_str))
}

fn build_globset(
    standard: &tanren_domain::methodology::standard::Standard,
) -> MethodologyResult<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for raw_pattern in &standard.applies_to {
        let normalized = normalize_path(raw_pattern);
        let glob = Glob::new(&normalized).map_err(|e| MethodologyError::FieldValidation {
            field_path: format!(
                "/standards/{}/{}/applies_to",
                standard.category.as_str(),
                standard.name.as_str()
            ),
            expected: "glob pattern accepted by `globset`".into(),
            actual: raw_pattern.clone(),
            remediation: format!(
                "fix invalid applies_to glob `{raw_pattern}` for standard {}:{} ({e})",
                standard.category.as_str(),
                standard.name.as_str()
            ),
        })?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|e| MethodologyError::FieldValidation {
            field_path: format!(
                "/standards/{}/{}/applies_to",
                standard.category.as_str(),
                standard.name.as_str()
            ),
            expected: "compilable globset".into(),
            actual: format!("{:?}", standard.applies_to),
            remediation: format!(
                "failed to compile applies_to globset for standard {}:{} ({e})",
                standard.category.as_str(),
                standard.name.as_str()
            ),
        })
}

fn normalize_path(input: &str) -> String {
    input.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn matcher_cache_key(standard: &tanren_domain::methodology::standard::Standard) -> String {
    format!(
        "{}:{}:{}",
        standard.category.as_str(),
        standard.name.as_str(),
        standard.applies_to.join("\u{1f}")
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EffectiveRelevanceInputs {
    touched_files: Vec<String>,
    project_languages: Vec<String>,
    domains: Vec<String>,
}

impl EffectiveRelevanceInputs {
    fn from_derived_and_hints(
        derived: &SpecRelevanceContext,
        hints: &ListRelevantStandardsParams,
    ) -> Self {
        let mut touched_files = derived.touched_files.clone();
        touched_files.extend(hints.touched_files.clone());
        touched_files.sort();
        touched_files.dedup();

        let mut project_languages = BTreeSet::new();
        if let Some(v) = derived.project_language.as_deref() {
            let normalized = normalize_match_label(v);
            if !normalized.is_empty() {
                project_languages.insert(normalized);
            }
        }
        if let Some(v) = hints.project_language.as_deref() {
            let normalized = normalize_match_label(v);
            if !normalized.is_empty() {
                project_languages.insert(normalized);
            }
        }
        let mut domains = BTreeSet::new();
        for tag in &derived.tags {
            let normalized = normalize_match_label(tag);
            if !normalized.is_empty() {
                domains.insert(normalized);
            }
        }
        if let Some(category) = derived.category.as_deref() {
            let normalized = normalize_match_label(category);
            if !normalized.is_empty() {
                domains.insert(normalized);
            }
        }
        for domain in &hints.domains {
            let normalized = normalize_match_label(domain);
            if !normalized.is_empty() {
                domains.insert(normalized);
            }
        }
        for tag in &hints.tags {
            let normalized = normalize_match_label(tag);
            if !normalized.is_empty() {
                domains.insert(normalized);
            }
        }
        if let Some(category) = hints.category.as_deref() {
            let normalized = normalize_match_label(category);
            if !normalized.is_empty() {
                domains.insert(normalized);
            }
        }

        Self {
            touched_files,
            project_languages: project_languages.into_iter().collect(),
            domains: domains.into_iter().collect(),
        }
    }
}

fn normalize_match_label(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}
