//! Deterministic spec-artifact projection from `phase-events.jsonl`.
use std::path::Path;

use super::artifact_projection_artifacts::{
    build_spec_frontmatter, render_audit_markdown, render_demo_markdown, render_signposts_markdown,
};
use super::artifact_projection_fold::{
    FoldedProjectionState, fold_projection_lines, fold_projection_lines_incremental,
};
use super::artifact_projection_helpers::{render_spec_body, write_artifacts};
use super::artifact_projection_render::{ProgressMetadata, render_task_projection_artifacts};
use super::errors::{MethodologyError, MethodologyResult};
use super::phase_events::{PHASE_EVENT_LINE_SCHEMA_VERSION, PhaseEventLine};
use super::service::MethodologyService;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::evidence::frontmatter::EvidenceSchemaVersion;
use tanren_domain::methodology::spec::{DemoEnvironment, SpecDependencies, SpecRelevanceContext};
use tanren_domain::methodology::task::{AcceptanceCriterion, RequiredGuard, Task, TaskGuardFlags};
use tanren_domain::{EventId, NonEmptyString, SpecId};

pub(super) const GENERATED_ARTIFACT_MANIFEST_FILE: &str = ".tanren-generated-artifacts.json";
pub(super) const PROJECTION_CHECKPOINT_FILE: &str = ".tanren-projection-checkpoint.json";
const ARTIFACT_CONTRACT_VERSION: &str = "v1";
const JSON_SCHEMA_VERSION: &str = "v1";
const PROJECTION_CHECKPOINT_SCHEMA_VERSION: &str = "v1";
const CHECKPOINT_COMPACTION_APPEND_THRESHOLD: usize = 200;
const GENERATED_ARTIFACTS: [&str; 9] = [
    "spec.md",
    "plan.md",
    "tasks.md",
    "tasks.json",
    "demo.md",
    "audit.md",
    "signposts.md",
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionCheckpoint {
    schema_version: String,
    contract_version: String,
    spec_id: SpecId,
    processed_lines: usize,
    last_event_id: Option<EventId>,
    compacted_at: DateTime<Utc>,
    state: FoldedProjectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TaskEvidence {
    pub(super) event_id: EventId,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) phase: String,
    pub(super) tool: String,
    pub(super) rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TaskProjectionRow {
    pub(super) task: Task,
    pub(super) guards: TaskGuardFlags,
    pub(super) evidence: TaskEvidence,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone)]
pub(super) struct RenderedArtifacts {
    pub(super) spec_md: String,
    pub(super) plan_md: String,
    pub(super) tasks_md: String,
    pub(super) tasks_json: String,
    pub(super) demo_md: String,
    pub(super) audit_md: String,
    pub(super) signposts_md: String,
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

        let raw =
            std::fs::read_to_string(&phase_events).map_err(|source| MethodologyError::Io {
                path: phase_events.clone(),
                source,
            })?;
        let raw_lines = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();

        let checkpoint_path = spec_folder.join(PROJECTION_CHECKPOINT_FILE);
        let prior_checkpoint = load_projection_checkpoint(&checkpoint_path);
        let (folded, compacted_at) = fold_with_optional_checkpoint(
            spec_id,
            &raw_lines,
            self.required_guards(),
            prior_checkpoint,
        )?;

        let rendered = render_from_folded(spec_id, &folded, self.required_guards())?;
        write_artifacts(spec_folder, rendered)?;

        persist_projection_checkpoint(
            &checkpoint_path,
            &ProjectionCheckpoint {
                schema_version: PROJECTION_CHECKPOINT_SCHEMA_VERSION.into(),
                contract_version: ARTIFACT_CONTRACT_VERSION.into(),
                spec_id,
                processed_lines: raw_lines.len(),
                last_event_id: folded.latest_event_id,
                compacted_at,
                state: folded,
            },
        )
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

fn load_projection_checkpoint(path: &Path) -> Option<ProjectionCheckpoint> {
    let raw = std::fs::read_to_string(path).ok()?;
    let checkpoint = serde_json::from_str::<ProjectionCheckpoint>(&raw).ok()?;
    if checkpoint.schema_version != PROJECTION_CHECKPOINT_SCHEMA_VERSION
        || checkpoint.contract_version != ARTIFACT_CONTRACT_VERSION
    {
        return None;
    }
    Some(checkpoint)
}

fn persist_projection_checkpoint(
    path: &Path,
    checkpoint: &ProjectionCheckpoint,
) -> MethodologyResult<()> {
    let bytes = serde_json::to_vec_pretty(&checkpoint)
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;
    write_atomic(path, &bytes)
}

fn fold_with_optional_checkpoint(
    spec_id: SpecId,
    raw_lines: &[&str],
    required_guards: &[RequiredGuard],
    prior_checkpoint: Option<ProjectionCheckpoint>,
) -> MethodologyResult<(FoldedProjectionState, DateTime<Utc>)> {
    if let Some(checkpoint) = prior_checkpoint {
        if checkpoint.spec_id == spec_id
            && checkpoint.processed_lines <= raw_lines.len()
            && checkpoint_anchor_matches(&checkpoint, raw_lines)
        {
            let appended = parse_phase_event_lines(raw_lines, checkpoint.processed_lines)?;
            let append_count = appended.len();
            let folded = fold_projection_lines_incremental(
                checkpoint.state,
                spec_id,
                &appended,
                required_guards,
            );
            let compacted_at = if append_count >= CHECKPOINT_COMPACTION_APPEND_THRESHOLD {
                folded.generated_at
            } else {
                checkpoint.compacted_at
            };
            return Ok((folded, compacted_at));
        }
    }

    let parsed = parse_phase_event_lines(raw_lines, 0)?;
    let folded = fold_projection_lines(spec_id, &parsed, required_guards);
    Ok((folded.clone(), folded.generated_at))
}

fn checkpoint_anchor_matches(checkpoint: &ProjectionCheckpoint, raw_lines: &[&str]) -> bool {
    if checkpoint.processed_lines == 0 {
        return true;
    }
    if checkpoint.processed_lines > raw_lines.len() {
        return false;
    }
    let Some(expected) = checkpoint.last_event_id else {
        return false;
    };
    let observed_line = raw_lines[checkpoint.processed_lines - 1];
    parse_event_id_from_raw_line(observed_line).is_some_and(|event_id| event_id == expected)
}

fn parse_event_id_from_raw_line(raw: &str) -> Option<EventId> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let id_raw = value.get("event_id")?.as_str()?;
    let id = uuid::Uuid::parse_str(id_raw).ok()?;
    Some(EventId::from_uuid(id))
}

#[cfg(test)]
fn read_phase_event_lines(path: &Path) -> MethodologyResult<Vec<PhaseEventLine>> {
    let raw = std::fs::read_to_string(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw_lines = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    parse_phase_event_lines(&raw_lines, 0)
}

fn parse_phase_event_lines(
    raw_lines: &[&str],
    start: usize,
) -> MethodologyResult<Vec<PhaseEventLine>> {
    let mut lines = Vec::new();
    for (idx, line) in raw_lines.iter().enumerate().skip(start) {
        let parsed = serde_json::from_str::<PhaseEventLine>(line).map_err(|err| {
            MethodologyError::FieldValidation {
                field_path: format!("/phase-events.jsonl:{}", idx + 1),
                expected: "valid phase-event JSON envelope".into(),
                actual: err.to_string(),
                remediation:
                    "run `tanren-cli methodology reconcile-phase-events` to repair projection artifacts"
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

#[cfg(test)]
fn render_from_lines(
    spec_id: SpecId,
    lines: &[PhaseEventLine],
    required_guards: &[RequiredGuard],
) -> MethodologyResult<RenderedArtifacts> {
    let folded = fold_projection_lines(spec_id, lines, required_guards);
    render_from_folded(spec_id, &folded, required_guards)
}

fn render_from_folded(
    spec_id: SpecId,
    folded: &FoldedProjectionState,
    _required_guards: &[RequiredGuard],
) -> MethodologyResult<RenderedArtifacts> {
    let progress_metadata = ProgressMetadata {
        first_event_at: folded.first_event_at,
        last_event_at: folded.last_event_at,
        latest_event_id: folded.latest_event_id,
        latest_phase: folded.latest_phase.clone(),
    };
    let generated_at = folded.generated_at;
    let spec_frontmatter = build_spec_frontmatter(spec_id, folded);
    let spec_markdown = spec_frontmatter
        .render_to_markdown(&render_spec_body(&spec_frontmatter))
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;
    let demo_md = render_demo_markdown(spec_id, generated_at, folded)?;
    let audit_md = render_audit_markdown(spec_id, generated_at, folded)?;
    let signposts_md = render_signposts_markdown(spec_id, folded)?;

    let tasks = folded.tasks.as_slice();

    let (plan_md, tasks_md, tasks_json, progress_json) = render_task_projection_artifacts(
        spec_id,
        generated_at,
        &spec_frontmatter,
        tasks,
        progress_metadata,
        JSON_SCHEMA_VERSION,
        ARTIFACT_CONTRACT_VERSION,
    )?;

    let manifest_json = serde_json::to_string_pretty(&generated_artifact_manifest())
        .map_err(|err| MethodologyError::Validation(err.to_string()))?;

    Ok(RenderedArtifacts {
        spec_md: spec_markdown,
        plan_md,
        tasks_md,
        tasks_json,
        demo_md,
        audit_md,
        signposts_md,
        progress_json,
        manifest_json,
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> MethodologyResult<()> {
    use std::io::Write as _;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut temp_path = path.to_path_buf();
    let file_name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("artifact");
    temp_path.set_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::now_v7()));

    let mut file = std::fs::File::create(&temp_path).map_err(|source| MethodologyError::Io {
        path: temp_path.clone(),
        source,
    })?;
    file.write_all(bytes)
        .map_err(|source| MethodologyError::Io {
            path: temp_path.clone(),
            source,
        })?;
    file.sync_all().map_err(|source| MethodologyError::Io {
        path: temp_path.clone(),
        source,
    })?;
    std::fs::rename(&temp_path, path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "artifact_projection_tests.rs"]
mod tests;
