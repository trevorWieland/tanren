//! Typed spec.md frontmatter.
//!
//! Shape mirrors `docs/architecture/evidence-schemas.md` §2 verbatim.
//! Construction is mediated exclusively by `spec.frontmatter` tool
//! calls; this module owns the wire contract.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::SpecId;
use crate::methodology::spec::{
    DemoEnvironment, SpecDependencies, SpecRelevanceContext, TouchedSymbol,
};
use crate::methodology::task::AcceptanceCriterion;
use crate::validated::NonEmptyString;

use super::frontmatter::{
    EvidenceSchemaVersion, FrontmatterError, default_schema_version, join, parse_typed,
};

/// Typed `spec.md` frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecFrontmatter {
    #[serde(default = "default_schema_version")]
    pub schema_version: EvidenceSchemaVersion,
    pub kind: SpecKind,
    pub spec_id: SpecId,
    pub title: NonEmptyString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem_statement: Option<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motivations: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expectations: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planned_behaviors: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_plan: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_negotiables: Vec<NonEmptyString>,
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default)]
    pub demo_environment: DemoEnvironment,
    #[serde(default)]
    pub dependencies: SpecDependencies,
    pub base_branch: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_symbols: Vec<TouchedSymbol>,
    #[serde(default, skip_serializing_if = "SpecRelevanceContext::is_empty")]
    pub relevance_context: SpecRelevanceContext,
    pub created_at: DateTime<Utc>,
}

/// Fixed discriminant tag for the spec frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    Spec,
}

impl SpecFrontmatter {
    /// Parse a `---\n<yaml>\n---\n<body>` document.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn parse_from_markdown(input: &str) -> Result<(Self, String), FrontmatterError> {
        parse_typed(input)
    }

    /// Render to a canonical `---\n<yaml>\n---\n<body>` document.
    ///
    /// # Errors
    /// See [`FrontmatterError`].
    pub fn render_to_markdown(&self, body: &str) -> Result<String, FrontmatterError> {
        join(self, body)
    }
}
