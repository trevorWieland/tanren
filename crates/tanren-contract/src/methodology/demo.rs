//! Wire contract for demo-frontmatter tools (§3.4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::methodology::evidence::demo::{DemoStatus, DemoStepMode};

use super::SchemaVersion;

/// `add_demo_step` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddDemoStepParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub id: String,
    pub mode: DemoStepMode,
    pub description: String,
    pub expected_observable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `mark_demo_step_skip` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkDemoStepSkipParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub step_id: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `append_demo_result` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppendDemoResultParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub step_id: String,
    pub status: DemoStatus,
    pub observed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
