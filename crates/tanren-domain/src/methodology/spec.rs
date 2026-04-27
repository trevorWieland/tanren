//! Spec entity — the top-level unit of planned work.
//!
//! A [`Spec`] is the aggregate that owns tasks, findings, signposts,
//! demo steps, and a rubric scorecard. Its frontmatter subset (see
//! `evidence::spec::SpecFrontmatter`) is the authoritative, tools-only
//! record of title, non-negotiables, acceptance criteria, demo
//! environment, dependencies, base branch, and touched symbols.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::SpecId;
use crate::validated::NonEmptyString;

use super::task::AcceptanceCriterion;

/// Top-level methodology aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Spec {
    pub id: SpecId,
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
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    pub demo_environment: DemoEnvironment,
    pub dependencies: SpecDependencies,
    pub base_branch: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_symbols: Vec<TouchedSymbol>,
    #[serde(default, skip_serializing_if = "SpecRelevanceContext::is_empty")]
    pub relevance_context: SpecRelevanceContext,
    pub created_at: DateTime<Utc>,
}

/// Spec metadata used to derive adherence relevance server-side.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpecRelevanceContext {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touched_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl SpecRelevanceContext {
    /// True when no relevance metadata has been set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.touched_files.is_empty()
            && self.project_language.is_none()
            && self.tags.is_empty()
            && self.category.is_none()
    }
}

/// Environmental connections the demo phase will probe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DemoEnvironment {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<DemoConnection>,
}

/// One probed connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DemoConnection {
    pub name: NonEmptyString,
    pub kind: ConnectionKind,
    pub probe: NonEmptyString,
}

/// Kinds of demo environment connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConnectionKind {
    Http,
    Postgres,
    Sqlite,
    Redis,
    Sqs,
    Kafka,
    Fs,
    /// Open-world extension: `custom:<name>`. The payload carries the
    /// suffix after the colon so typed consumers can dispatch by name.
    Custom(String),
}

/// Cross-spec and external dependencies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpecDependencies {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on_spec_ids: Vec<SpecId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_issue_refs: Vec<NonEmptyString>,
}

/// Touched-symbol reference for cross-spec concern resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TouchedSymbol {
    pub kind: SymbolKind,
    pub name: NonEmptyString,
}

/// Kinds of touched symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Module,
    Fn,
    Type,
    Trait,
    Macro,
}
