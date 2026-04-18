//! Wire contract for spec-frontmatter tools (§3.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::methodology::spec::{DemoEnvironment, SpecDependencies};
use tanren_domain::methodology::task::AcceptanceCriterion;

/// `set_spec_title` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecTitleParams {
    pub spec_id: SpecId,
    pub title: String,
}

/// `set_spec_non_negotiables` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecNonNegotiablesParams {
    pub spec_id: SpecId,
    pub items: Vec<String>,
}

/// `add_spec_acceptance_criterion` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddSpecAcceptanceCriterionParams {
    pub spec_id: SpecId,
    pub criterion: AcceptanceCriterion,
}

/// `set_spec_demo_environment` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecDemoEnvironmentParams {
    pub spec_id: SpecId,
    pub demo_environment: DemoEnvironment,
}

/// `set_spec_dependencies` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecDependenciesParams {
    pub spec_id: SpecId,
    pub dependencies: SpecDependencies,
}

/// `set_spec_base_branch` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetSpecBaseBranchParams {
    pub spec_id: SpecId,
    pub branch: String,
}
