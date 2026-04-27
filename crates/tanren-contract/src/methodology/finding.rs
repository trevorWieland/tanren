//! Wire contract for `add_finding` and `record_adherence_finding` (§3.2, §3.8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::check::CheckKind;
use tanren_domain::methodology::finding::{
    AdherenceSeverity, FindingLifecycleEvidence, FindingSeverity, FindingSource, FindingStatus,
    FindingView, StandardRef,
};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::{FindingId, SpecId, TaskId};

use super::SchemaVersion;

/// `add_finding` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddFindingParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub severity: FindingSeverity,
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_numbers: Vec<u32>,
    pub source: FindingSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_task: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `add_finding` / `record_adherence_finding` shared response shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddFindingResponse {
    pub schema_version: SchemaVersion,
    pub finding_id: FindingId,
}

/// Scope filter for finding list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingScopeFilter {
    Spec,
    Task,
}

/// `finding list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListFindingsParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<FindingStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<FindingSeverity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<FindingScopeFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_kind: Option<CheckKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_phase: Option<PhaseId>,
}

/// `finding list` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListFindingsResponse {
    pub schema_version: SchemaVersion,
    pub findings: Vec<FindingView>,
}

/// Shared params for finding lifecycle commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FindingLifecycleParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub finding_id: FindingId,
    pub evidence: FindingLifecycleEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `finding supersede` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupersedeFindingParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub finding_id: FindingId,
    pub superseded_by: Vec<FindingId>,
    pub evidence: FindingLifecycleEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `record_adherence_finding` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordAdherenceFindingParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub standard: StandardRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_numbers: Vec<u32>,
    pub severity: AdherenceSeverity,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_task: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
