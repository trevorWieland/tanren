//! Wire contract for task-lifecycle tools (§3.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::task::{
    AcceptanceCriterion, ExplicitUserDiscardProvenance, RequiredGuard, Task,
    TaskAbandonDisposition, TaskOrigin,
};
use tanren_domain::{SpecId, TaskId};

use super::SchemaVersion;

/// `create_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTaskParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
    pub origin: TaskOrigin,
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `create_task` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTaskResponse {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
}

/// `start_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartTaskParams {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `complete_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompleteTaskParams {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `revise_task` params. Non-transitional — mutates description /
/// acceptance criteria only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviseTaskParams {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
    pub revised_description: String,
    pub revised_acceptance: Vec<AcceptanceCriterion>,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `abandon_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AbandonTaskParams {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
    pub reason: String,
    pub disposition: TaskAbandonDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replacements: Vec<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_user_discard_provenance: Option<ExplicitUserDiscardProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `mark_task_guard_satisfied` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkTaskGuardSatisfiedParams {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
    pub guard: RequiredGuard,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `reset_task_guards` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetTaskGuardsParams {
    pub schema_version: SchemaVersion,
    pub task_id: TaskId,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `list_tasks` filter.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTasksParams {
    pub schema_version: SchemaVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_id: Option<SpecId>,
}

/// `list_tasks` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTasksResponse {
    pub schema_version: SchemaVersion,
    pub tasks: Vec<Task>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::schema_for;

    #[test]
    fn create_task_params_schema_contains_title_field() {
        let schema = schema_for!(CreateTaskParams);
        let json = serde_json::to_string(&schema).expect("serialize schema");
        assert!(
            json.contains("\"title\""),
            "schema JSON must mention the `title` field: {json}"
        );
    }
}
