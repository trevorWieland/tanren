//! Wire contract for `add_finding` and `record_adherence_finding` (§3.2, §3.8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::finding::{FindingSeverity, FindingSource, StandardRef};
use tanren_domain::{FindingId, SpecId, TaskId};

/// `add_finding` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddFindingParams {
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
}

/// `add_finding` / `record_adherence_finding` shared response shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddFindingResponse {
    pub finding_id: FindingId,
}

/// `record_adherence_finding` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecordAdherenceFindingParams {
    pub spec_id: SpecId,
    pub standard: StandardRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_numbers: Vec<u32>,
    pub severity: FindingSeverity,
    pub rationale: String,
}
