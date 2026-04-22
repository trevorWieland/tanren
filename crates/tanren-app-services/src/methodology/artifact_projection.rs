//! Deterministic spec-artifact projection from `phase-events.jsonl`.
use std::path::Path;

use super::artifact_projection_fold::fold_projection_lines;
use super::artifact_projection_helpers::{
    count_tasks, owner_phase_label, render_demo_body, render_plan_body, render_spec_body,
    render_tasks_markdown, write_artifacts,
};
use super::errors::{MethodologyError, MethodologyResult};
use super::phase_events::{PHASE_EVENT_LINE_SCHEMA_VERSION, PhaseEventLine};
use super::service::MethodologyService;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tanren_domain::methodology::evidence::demo::{DemoEnvironmentProbe, DemoFrontmatter, DemoKind};
use tanren_domain::methodology::evidence::frontmatter::EvidenceSchemaVersion;
use tanren_domain::methodology::evidence::plan::{PlanFrontmatter, PlanKind};
use tanren_domain::methodology::evidence::spec::{SpecFrontmatter, SpecKind};
use tanren_domain::methodology::spec::{DemoEnvironment, SpecDependencies, SpecRelevanceContext};
use tanren_domain::methodology::task::{AcceptanceCriterion, RequiredGuard, Task, TaskGuardFlags};
use tanren_domain::{EventId, NonEmptyString, SpecId};

pub(super) const GENERATED_ARTIFACT_MANIFEST_FILE: &str = ".tanren-generated-artifacts.json";
const ARTIFACT_CONTRACT_VERSION: &str = "v1";
const JSON_SCHEMA_VERSION: &str = "v1";
const GENERATED_ARTIFACTS: [&str; 7] = [
    "spec.md",
    "plan.md",
    "tasks.md",
    "tasks.json",
    "demo.md",
    "progress.json",
    "phase-events.jsonl",
];
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GeneratedArtifactManifest {
    pub schema_version: String,
    pub contract_version: String,
    pub generated_artifacts: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct TaskEvidence {
    pub(super) event_id: EventId,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) phase: String,
    pub(super) tool: String,
    pub(super) rationale: String,
}

#[derive(Debug, Clone)]
pub(super) struct TaskProjectionRow {
    pub(super) task: Task,
    pub(super) guards: TaskGuardFlags,
    pub(super) evidence: TaskEvidence,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SpecState {
    pub(super) title: Option<NonEmptyString>,
    pub(super) problem_statement: Option<NonEmptyString>,
    pub(super) motivations: Vec<NonEmptyString>,
    pub(super) expectations: Vec<NonEmptyString>,
    pub(super) planned_behaviors: Vec<NonEmptyString>,
    pub(super) implementation_plan: Vec<NonEmptyString>,
    pub(super) non_negotiables: Vec<NonEmptyString>,
    pub(super) acceptance_criteria: Vec<AcceptanceCriterion>,
    pub(super) demo_environment: DemoEnvironment,
    pub(super) dependencies: SpecDependencies,
    pub(super) base_branch: Option<NonEmptyString>,
    pub(super) relevance_context: SpecRelevanceContext,
    pub(super) created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TasksFrontmatter {
    #[serde(default = "default_schema_version")]
    pub(super) schema_version: EvidenceSchemaVersion,
    pub(super) kind: TasksKind,
    pub(super) spec_id: SpecId,
    pub(super) generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TasksKind {
    Tasks,
}

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
struct ProgressMetadata {
    first_event_at: Option<DateTime<Utc>>,
    last_event_at: Option<DateTime<Utc>>,
    latest_event_id: Option<EventId>,
    latest_phase: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RenderedArtifacts {
    pub(super) spec_md: String,
    pub(super) plan_md: String,
    pub(super) tasks_md: String,
    pub(super) tasks_json: String,
    pub(super) demo_md: String,
    pub(super) progress_json: String,
    pub(super) manifest_json: String,
}

impl MethodologyService {
    pub(crate) fn materialize_projected_artifacts(
        &self,
        spec_id: SpecId,
        spec_folder: &Path,
    ) -> MethodologyResult<()> {
        let phase_events = spec_folder.join("phase-events.jsonl");
        if !phase_events.exists() {
            return Ok(());
        }
        let lines = read_phase_event_lines(&phase_events)?;
        let rendered = render_from_lines(spec_id, &lines, self.required_guards())?;
        write_artifacts(spec_folder, rendered)
    }
}

pub(super) fn generated_artifact_manifest() -> GeneratedArtifactManifest {
    GeneratedArtifactManifest {
        schema_version: JSON_SCHEMA_VERSION.into(),
        contract_version: ARTIFACT_CONTRACT_VERSION.into(),
        generated_artifacts: GENERATED_ARTIFACTS
            .iter()
            .map(|v| (*v).to_owned())
            .collect(),
    }
}

fn read_phase_event_lines(path: &Path) -> MethodologyResult<Vec<PhaseEventLine>> {
    let raw = std::fs::read_to_string(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut lines = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<PhaseEventLine>(line).map_err(|err| {
            MethodologyError::FieldValidation {
                field_path: format!("/phase-events.jsonl:{}", idx + 1),
                expected: "valid phase-event JSON envelope".into(),
                actual: err.to_string(),
                remediation:
                    "run `tanren methodology reconcile-phase-events` to repair projection artifacts"
                        .into(),
            }
        })?;
        if parsed.schema_version != PHASE_EVENT_LINE_SCHEMA_VERSION {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/phase-events.jsonl:{}/schema_version", idx + 1),
                expected: PHASE_EVENT_LINE_SCHEMA_VERSION.into(),
                actual: parsed.schema_version.clone(),
                remediation:
                    "regenerate phase-events envelopes so every line declares schema_version=1.0.0"
                        .into(),
            });
        }
        lines.push(parsed);
    }
    Ok(lines)
}

fn render_from_lines(
    spec_id: SpecId,
    lines: &[PhaseEventLine],
    required_guards: &[RequiredGuard],
) -> MethodologyResult<RenderedArtifacts> {
    let folded = fold_projection_lines(spec_id, lines, required_guards);
    let progress_metadata = ProgressMetadata {
        first_event_at: folded.first_event_at,
        last_event_at: folded.last_event_at,
        latest_event_id: folded.latest_event_id,
        latest_phase: folded.latest_phase.clone(),
    };
    let generated_at = folded.generated_at;
    let title = folded
        .spec_state
        .title
        .unwrap_or_else(|| NonEmptyString::try_new("Untitled spec").expect("non-empty literal"));
    let base_branch = folded
        .spec_state
        .base_branch
        .unwrap_or_else(|| NonEmptyString::try_new("main").expect("non-empty literal"));
    let created_at = folded.spec_state.created_at.unwrap_or(generated_at);

    let spec_frontmatter = SpecFrontmatter {
        schema_version: EvidenceSchemaVersion::current(),
        kind: SpecKind::Spec,
        spec_id,
        title,
        problem_statement: folded.spec_state.problem_statement,
        motivations: folded.spec_state.motivations.clone(),
        expectations: folded.spec_state.expectations.clone(),
        planned_behaviors: folded.spec_state.planned_behaviors.clone(),
        implementation_plan: folded.spec_state.implementation_plan.clone(),
        non_negotiables: folded.spec_state.non_negotiables.clone(),
        acceptance_criteria: folded.spec_state.acceptance_criteria.clone(),
        demo_environment: folded.spec_state.demo_environment.clone(),
        dependencies: folded.spec_state.dependencies,
        base_branch,
        touched_symbols: vec![],
        relevance_context: folded.spec_state.relevance_context,
        created_at,
    };
    let spec_markdown = spec_frontmatter
        .render_to_markdown(&render_spec_body(&spec_frontmatter))
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;

    let demo_frontmatter = DemoFrontmatter {
        schema_version: EvidenceSchemaVersion::current(),
        kind: DemoKind::Demo,
        spec_id,
        environment: DemoEnvironmentProbe {
            probed_at: folded.last_demo_mutation.unwrap_or(generated_at),
            connections_verified: !folded.demo_steps.is_empty() || !folded.demo_results.is_empty(),
        },
        steps: folded.demo_steps.clone(),
        results: folded.demo_results.clone(),
    };
    let demo_md = demo_frontmatter
        .render_to_markdown(&render_demo_body(&demo_frontmatter))
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;

    let tasks = folded.tasks.as_slice();

    let (plan_md, tasks_md, tasks_json, progress_json) = render_task_projection_artifacts(
        spec_id,
        generated_at,
        &spec_frontmatter,
        tasks,
        progress_metadata,
    )?;

    let manifest_json = serde_json::to_string_pretty(&generated_artifact_manifest())
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;

    Ok(RenderedArtifacts {
        spec_md: spec_markdown,
        plan_md,
        tasks_md,
        tasks_json,
        demo_md,
        progress_json,
        manifest_json,
    })
}

fn render_task_projection_artifacts(
    spec_id: SpecId,
    generated_at: DateTime<Utc>,
    spec_frontmatter: &SpecFrontmatter,
    tasks: &[TaskProjectionRow],
    progress_metadata: ProgressMetadata,
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
        schema_version: JSON_SCHEMA_VERSION.into(),
        contract_version: ARTIFACT_CONTRACT_VERSION.into(),
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
        schema_version: JSON_SCHEMA_VERSION.into(),
        contract_version: ARTIFACT_CONTRACT_VERSION.into(),
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

#[cfg(test)]
#[path = "artifact_projection_tests.rs"]
mod tests;
