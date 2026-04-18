//! Wire contract for standards tools (§3.8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;

use super::SchemaVersion;

/// `list_relevant_standards` params.
///
/// Filter inputs drive the adherence §4.1 relevance algorithm:
/// - `touched_files` is matched against each standard's `applies_to`
///   glob list.
/// - `project_language` is matched against `applies_to_languages`.
/// - `domains` is matched against `applies_to_domains`.
///
/// Any filter input left empty means "no constraint from this axis".
/// All-empty inputs fall back to the baseline-complete upper bound
/// (backward compatible with pre-Lane-0.5 callers).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListRelevantStandardsParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<String>,
}

/// One entry in the `list_relevant_standards` response. The reason
/// field makes relevance inclusion explainable; operators can see which
/// filter axis matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelevantStandard {
    pub schema_version: SchemaVersion,
    pub standard: tanren_domain::methodology::standard::Standard,
    /// Human-readable reason the standard was included.
    pub inclusion_reason: String,
}
