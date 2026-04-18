//! Typed demo.md frontmatter.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::SpecId;
use crate::validated::NonEmptyString;

use super::frontmatter::{FrontmatterError, join, parse_typed};

/// Typed `demo.md` frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DemoFrontmatter {
    pub kind: DemoKind,
    pub spec_id: SpecId,
    pub environment: DemoEnvironmentProbe,
    #[serde(default)]
    pub steps: Vec<DemoStep>,
    /// Append-only. New runs push new entries; existing entries are
    /// immutable. Enforced at tool call in `app-services`.
    #[serde(default)]
    pub results: Vec<DemoResult>,
}

/// Fixed discriminant tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DemoKind {
    Demo,
}

/// Environment probe stamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DemoEnvironmentProbe {
    pub probed_at: DateTime<Utc>,
    pub connections_verified: bool,
}

/// One declared demo step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DemoStep {
    pub id: NonEmptyString,
    pub mode: DemoStepMode,
    pub description: NonEmptyString,
    pub expected_observable: NonEmptyString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<NonEmptyString>,
}

/// Run mode for a demo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum DemoStepMode {
    #[serde(rename = "RUN")]
    Run,
    #[serde(rename = "SKIP")]
    Skip,
}

/// One appended demo-run observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DemoResult {
    pub run_id: crate::ids::EventId,
    pub ran_at: DateTime<Utc>,
    pub step_id: NonEmptyString,
    pub status: DemoStatus,
    pub observed: String,
}

/// Pass/fail status of one demo step in one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DemoStatus {
    Pass,
    Fail,
}

impl DemoFrontmatter {
    /// Parse.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn parse_from_markdown(input: &str) -> Result<(Self, String), FrontmatterError> {
        parse_typed(input)
    }

    /// Render.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn render_to_markdown(&self, body: &str) -> Result<String, FrontmatterError> {
        join(self, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_serde_uppercase() {
        let json = serde_json::to_string(&DemoStepMode::Run).expect("serialize");
        assert_eq!(json, "\"RUN\"");
        let json = serde_json::to_string(&DemoStepMode::Skip).expect("serialize");
        assert_eq!(json, "\"SKIP\"");
    }

    #[test]
    fn roundtrip_stable() {
        let d = DemoFrontmatter {
            kind: DemoKind::Demo,
            spec_id: SpecId::new(),
            environment: DemoEnvironmentProbe {
                probed_at: Utc::now(),
                connections_verified: true,
            },
            steps: vec![DemoStep {
                id: NonEmptyString::try_new("step-1").expect("non-empty"),
                mode: DemoStepMode::Run,
                description: NonEmptyString::try_new("Probe the API").expect("non-empty"),
                expected_observable: NonEmptyString::try_new("HTTP 200").expect("non-empty"),
                skip_reason: None,
            }],
            results: vec![],
        };
        let doc = d.render_to_markdown("demo body\n").expect("render");
        let (back, body) = DemoFrontmatter::parse_from_markdown(&doc).expect("parse");
        assert_eq!(back, d);
        assert_eq!(body, "demo body\n");
    }
}
