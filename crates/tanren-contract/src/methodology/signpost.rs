//! Wire contract for signpost tools (§3.5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::signpost::SignpostStatus;
use tanren_domain::{SignpostId, SpecId, TaskId};

use super::SchemaVersion;

/// `add_signpost` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddSignpostParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub status: SignpostStatus,
    pub problem: String,
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tried: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_affected: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `add_signpost` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddSignpostResponse {
    pub schema_version: SchemaVersion,
    pub signpost_id: SignpostId,
}

/// `update_signpost_status` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateSignpostStatusParams {
    pub schema_version: SchemaVersion,
    pub signpost_id: SignpostId,
    pub status: SignpostStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
