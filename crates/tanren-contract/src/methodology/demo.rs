//! Wire contract for demo-frontmatter tools (§3.4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::methodology::evidence::demo::{DemoStatus, DemoStepMode};

/// `add_demo_step` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddDemoStepParams {
    pub spec_id: SpecId,
    pub id: String,
    pub mode: DemoStepMode,
    pub description: String,
    pub expected_observable: String,
}

/// `mark_demo_step_skip` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MarkDemoStepSkipParams {
    pub spec_id: SpecId,
    pub step_id: String,
    pub reason: String,
}

/// `append_demo_result` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppendDemoResultParams {
    pub spec_id: SpecId,
    pub step_id: String,
    pub status: DemoStatus,
    pub observed: String,
}
