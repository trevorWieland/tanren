//! Typed audit.md frontmatter.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{FindingId, SpecId};
use crate::methodology::pillar::PillarScope;
use crate::methodology::rubric::{NonNegotiableCompliance, RubricScore};

use super::frontmatter::{FrontmatterError, join, parse_typed};

/// Typed `audit.md` frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditFrontmatter {
    pub kind: AuditKind,
    pub spec_id: SpecId,
    pub scope: PillarScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_target_id: Option<String>,
    pub status: AuditStatus,
    pub fix_now_count: u32,
    #[serde(default)]
    pub rubric: Vec<RubricScore>,
    #[serde(default)]
    pub non_negotiables_compliance: Vec<NonNegotiableCompliance>,
    #[serde(default)]
    pub findings: Vec<FindingId>,
    pub generated_at: DateTime<Utc>,
}

/// Fixed discriminant tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    Audit,
}

/// Overall pass/fail status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditStatus {
    Pass,
    Fail,
}

impl AuditFrontmatter {
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
