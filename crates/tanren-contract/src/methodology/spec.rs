//! Wire contract for spec-frontmatter tools (§3.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::methodology::spec::{DemoEnvironment, SpecDependencies, SpecRelevanceContext};
use tanren_domain::methodology::task::AcceptanceCriterion;

use super::SchemaVersion;

/// `set_spec_title` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecTitleParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_non_negotiables` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecNonNegotiablesParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub items: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `add_spec_acceptance_criterion` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddSpecAcceptanceCriterionParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub criterion: AcceptanceCriterion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_demo_environment` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecDemoEnvironmentParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub demo_environment: DemoEnvironment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_dependencies` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecDependenciesParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub dependencies: SpecDependencies,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_base_branch` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecBaseBranchParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_relevance_context` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecRelevanceContextParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub relevance_context: SpecRelevanceContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
