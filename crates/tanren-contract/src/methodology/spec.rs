//! Wire contract for spec-frontmatter tools (§3.3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;
use tanren_domain::TaskId;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::phase_outcome::BlockedReason;
use tanren_domain::methodology::spec::{DemoEnvironment, SpecDependencies, SpecRelevanceContext};
use tanren_domain::methodology::task::{AcceptanceCriterion, RequiredGuard};
use tanren_domain::validated::NonEmptyString;

use super::SchemaVersion;

/// `spec_status` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecStatusParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
}

/// Canonical transition selected by the methodology state machine.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SpecStatusTransition {
    ShapeSpecRequired,
    TaskDo,
    TaskCheckBatch,
    TaskInvestigate,
    SpecCheckBatch,
    SpecInvestigate,
    ResolveBlockersRequired,
    WalkSpecRequired,
    Complete,
}

/// Typed spec-level check identity used by spec-check batching.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SpecCheckKind {
    SpecGate,
    RunDemo,
    AuditSpec,
    AdhereSpec,
}

/// Compact outcome tag for investigate-source metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhaseOutcomeTag {
    Complete,
    Blocked,
    Error,
}

/// `spec_status` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpecStatusResponse {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub spec_exists: bool,
    pub blockers_active: bool,
    pub ready_for_walk_spec: bool,
    pub next_transition: SpecStatusTransition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_task_checks: Vec<RequiredGuard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_spec_checks: Vec<SpecCheckKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub investigate_source_phase: Option<PhaseId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub investigate_source_outcome: Option<PhaseOutcomeTag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub investigate_source_summary: Option<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub investigate_source_task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_blocker_phase: Option<PhaseId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_blocker_summary: Option<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_blocker_reason: Option<BlockedReason>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_guards: Vec<RequiredGuard>,
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub abandoned_tasks: u64,
    pub implemented_tasks: u64,
    pub in_progress_tasks: u64,
    pub pending_tasks: u64,
}

/// `set_spec_title` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecTitleParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_problem_statement` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecProblemStatementParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub problem_statement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_motivations` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecMotivationsParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub motivations: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_expectations` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecExpectationsParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub expectations: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_planned_behaviors` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecPlannedBehaviorsParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub planned_behaviors: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_implementation_plan` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecImplementationPlanParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub implementation_plan: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_non_negotiables` params (full replacement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecNonNegotiablesParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub items: Vec<NonEmptyString>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `add_spec_acceptance_criterion` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddSpecAcceptanceCriterionParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub criterion: AcceptanceCriterion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_demo_environment` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecDemoEnvironmentParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub demo_environment: DemoEnvironment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_dependencies` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecDependenciesParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub dependencies: SpecDependencies,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_base_branch` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecBaseBranchParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `set_spec_relevance_context` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSpecRelevanceContextParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub relevance_context: SpecRelevanceContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
