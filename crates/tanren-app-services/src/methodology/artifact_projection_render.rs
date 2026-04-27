use chrono::{DateTime, Utc};
use serde::Serialize;
use tanren_domain::methodology::evidence::frontmatter::EvidenceSchemaVersion;
use tanren_domain::methodology::evidence::plan::{PlanFrontmatter, PlanKind};
use tanren_domain::methodology::evidence::spec::SpecFrontmatter;
use tanren_domain::methodology::task::Task;
use tanren_domain::{EventId, SpecId};

use super::artifact_projection::{TaskProjectionRow, TasksFrontmatter, TasksKind};
use super::artifact_projection_helpers::{
    count_tasks, owner_phase_label, render_plan_body, render_tasks_markdown,
};
use super::errors::{MethodologyError, MethodologyResult};

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct TasksJsonDocument {
    schema_version: String,
    contract_version: String,
    spec_id: SpecId,
    generated_at: DateTime<Utc>,
    tasks: Vec<TasksJsonRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct TasksJsonRow {
    task: Task,
    owner_phase: String,
    status_rationale: String,
    status_event_id: EventId,
    status_event_timestamp: DateTime<Utc>,
    status_event_phase: String,
    status_event_tool: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct ProgressJsonDocument {
    schema_version: String,
    contract_version: String,
    spec_id: SpecId,
    generated_at: DateTime<Utc>,
    task_counts: TaskCounts,
    first_event_at: Option<DateTime<Utc>>,
    last_event_at: Option<DateTime<Utc>>,
    latest_event_id: Option<EventId>,
    latest_phase: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TaskCounts {
    pub(super) pending: usize,
    pub(super) in_progress: usize,
    pub(super) implemented: usize,
    pub(super) complete: usize,
    pub(super) abandoned: usize,
    pub(super) total: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ProgressMetadata {
    pub(super) first_event_at: Option<DateTime<Utc>>,
    pub(super) last_event_at: Option<DateTime<Utc>>,
    pub(super) latest_event_id: Option<EventId>,
    pub(super) latest_phase: Option<String>,
}

pub(super) fn render_task_projection_artifacts(
    spec_id: SpecId,
    generated_at: DateTime<Utc>,
    spec_frontmatter: &SpecFrontmatter,
    tasks: &[TaskProjectionRow],
    progress_metadata: ProgressMetadata,
    json_schema_version: &str,
    artifact_contract_version: &str,
) -> MethodologyResult<(String, String, String, String)> {
    let plan_frontmatter = PlanFrontmatter {
        schema_version: EvidenceSchemaVersion::current(),
        kind: PlanKind::Plan,
        spec_id,
        generated_at,
    };
    let plan_md = plan_frontmatter
        .render_to_markdown(&render_plan_body(spec_frontmatter, tasks))
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;
    let tasks_frontmatter = TasksFrontmatter {
        schema_version: EvidenceSchemaVersion::current(),
        kind: TasksKind::Tasks,
        spec_id,
        generated_at,
    };
    let tasks_md = render_tasks_markdown(&tasks_frontmatter, tasks)
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;
    let tasks_json_doc = TasksJsonDocument {
        schema_version: json_schema_version.to_owned(),
        contract_version: artifact_contract_version.to_owned(),
        spec_id,
        generated_at,
        tasks: tasks
            .iter()
            .map(|row| TasksJsonRow {
                task: row.task.clone(),
                owner_phase: owner_phase_label(&row.task.origin),
                status_rationale: row.evidence.rationale.clone(),
                status_event_id: row.evidence.event_id,
                status_event_timestamp: row.evidence.timestamp,
                status_event_phase: row.evidence.phase.clone(),
                status_event_tool: row.evidence.tool.clone(),
            })
            .collect(),
    };
    let tasks_json = serde_json::to_string_pretty(&tasks_json_doc)
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;
    let progress_doc = ProgressJsonDocument {
        schema_version: json_schema_version.to_owned(),
        contract_version: artifact_contract_version.to_owned(),
        spec_id,
        generated_at,
        task_counts: count_tasks(tasks),
        first_event_at: progress_metadata.first_event_at,
        last_event_at: progress_metadata.last_event_at,
        latest_event_id: progress_metadata.latest_event_id,
        latest_phase: progress_metadata.latest_phase,
    };
    let progress_json = serde_json::to_string_pretty(&progress_doc)
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;
    Ok((plan_md, tasks_md, tasks_json, progress_json))
}
