//! Typed plan.md frontmatter.
//!
//! `plan.md` is **orchestrator-owned** — agents never author it. The
//! installer and the enforcement guard treat it as read-only. This
//! module exists so the orchestrator's own render path goes through a
//! typed contract identical in shape to the agent-authored files.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::SpecId;

use super::frontmatter::{
    EvidenceSchemaVersion, FrontmatterError, default_schema_version, join, parse_typed,
};

/// Typed `plan.md` frontmatter. Body is always orchestrator-generated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanFrontmatter {
    #[serde(default = "default_schema_version")]
    pub schema_version: EvidenceSchemaVersion,
    pub kind: PlanKind,
    pub spec_id: SpecId,
    pub generated_at: DateTime<Utc>,
}

/// Fixed discriminant tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    Plan,
}

impl PlanFrontmatter {
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
    fn roundtrip_stable() {
        let p = PlanFrontmatter {
            schema_version: EvidenceSchemaVersion::current(),
            kind: PlanKind::Plan,
            spec_id: SpecId::new(),
            generated_at: Utc::now(),
        };
        let doc = p.render_to_markdown("generated body\n").expect("render");
        let (back, body) = PlanFrontmatter::parse_from_markdown(&doc).expect("parse");
        assert_eq!(back, p);
        assert_eq!(body, "generated body\n");
    }
}
