//! Wire contract for `add_finding` and `record_adherence_finding` (§3.2, §3.8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::finding::{
    AdherenceSeverity, FindingSeverity, FindingSource, StandardRef,
};
use tanren_domain::{FindingId, SpecId, TaskId};

use super::SchemaVersion;

/// `add_finding` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
pub struct AddFindingResponse {
    pub schema_version: SchemaVersion,
    pub finding_id: FindingId,
}

/// `record_adherence_finding` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    pub idempotency_key: Option<String>,
}
