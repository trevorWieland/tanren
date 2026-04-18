//! Wire contract for task-lifecycle tools (§3.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::task::{AcceptanceCriterion, TaskOrigin};
use tanren_domain::{SpecId, TaskId};

/// `create_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateTaskParams {
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
}

/// `create_task` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateTaskResponse {
    pub task_id: TaskId,
}

/// `start_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StartTaskParams {
    pub task_id: TaskId,
}

/// `complete_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompleteTaskParams {
    pub task_id: TaskId,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

/// `revise_task` params. Non-transitional — mutates description /
/// acceptance criteria only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviseTaskParams {
    pub task_id: TaskId,
    pub revised_description: String,
    pub revised_acceptance: Vec<AcceptanceCriterion>,
    pub reason: String,
}

/// `abandon_task` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AbandonTaskParams {
    pub task_id: TaskId,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replacements: Vec<TaskId>,
}

/// `list_tasks` filter.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListTasksParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_id: Option<SpecId>,
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
