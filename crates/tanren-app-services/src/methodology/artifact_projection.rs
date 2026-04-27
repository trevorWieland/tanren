//! Deterministic spec-artifact projection from `phase-events.jsonl`.
use std::path::Path;

#[path = "artifact_projection_incremental.rs"]
mod artifact_projection_incremental;

use self::artifact_projection_incremental::fold_phase_events_file_with_optional_checkpoint;
use super::artifact_contract;
use super::artifact_projection_artifacts::{
    build_spec_frontmatter, render_audit_markdown, render_demo_markdown, render_signposts_markdown,
};
use super::artifact_projection_fold::FoldedProjectionState;
use super::artifact_projection_helpers::{render_spec_body, write_artifacts, write_if_changed};
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

pub(super) const GENERATED_ARTIFACT_MANIFEST_FILE: &str =
    artifact_contract::GENERATED_ARTIFACT_MANIFEST_FILE;
pub(super) const PROJECTION_CHECKPOINT_FILE: &str = artifact_contract::PROJECTION_CHECKPOINT_FILE;
const ARTIFACT_CONTRACT_VERSION: &str = "v1";
const JSON_SCHEMA_VERSION: &str = "v1";
const PROJECTION_CHECKPOINT_SCHEMA_VERSION: &str = "v1";
pub(super) const CHECKPOINT_ANCHOR_LOOKBACK_BYTES: u64 = 65_536;
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GeneratedArtifactManifest {
    pub schema_version: String,
    pub contract_version: String,
    pub generated_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectionCheckpoint {
    pub(super) schema_version: String,
    pub(super) contract_version: String,
    pub(super) spec_id: SpecId,
    pub(super) processed_lines: usize,
    #[serde(default)]
    pub(super) processed_bytes: u64,
    pub(super) last_event_id: Option<EventId>,
    pub(super) compacted_at: DateTime<Utc>,
    #[serde(default)]
    pub(super) compacted_line_count: usize,
    pub(super) state: FoldedProjectionState,
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

        let checkpoint_path = spec_folder.join(PROJECTION_CHECKPOINT_FILE);
        let prior_checkpoint = load_projection_checkpoint(&checkpoint_path);
        let append_threshold = self
            .runtime_tuning()
            .projection_checkpoint_compaction_append_threshold
            .max(1);
        let fold_result = fold_phase_events_file_with_optional_checkpoint(
            spec_id,
            &phase_events,
            self.required_guards(),
            prior_checkpoint,
            append_threshold,
        )?;
        let folded = fold_result.folded;

        let rendered = render_from_folded(spec_id, &folded, self.required_guards())?;
        write_artifacts(spec_folder, rendered)?;

        persist_projection_checkpoint(
            &checkpoint_path,
            &ProjectionCheckpoint {
                schema_version: PROJECTION_CHECKPOINT_SCHEMA_VERSION.into(),
                contract_version: ARTIFACT_CONTRACT_VERSION.into(),
                spec_id,
                processed_lines: fold_result.processed_lines,
                processed_bytes: fold_result.processed_bytes,
                last_event_id: folded.latest_event_id,
                compacted_at: fold_result.compacted_at,
                compacted_line_count: fold_result.compacted_line_count,
                state: folded,
            },
        )
    }
}

pub(super) fn generated_artifact_manifest() -> GeneratedArtifactManifest {
    GeneratedArtifactManifest {
        schema_version: JSON_SCHEMA_VERSION.into(),
        contract_version: ARTIFACT_CONTRACT_VERSION.into(),
        generated_artifacts: artifact_contract::generated_manifest_artifacts()
            .into_iter()
            .map(str::to_owned)
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
    let _ = write_if_changed(path, &bytes)?;
    Ok(())
}

pub(super) fn parse_event_id_from_raw_line(raw: &str) -> Option<EventId> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let id_raw = value.get("event_id")?.as_str()?;
    let id = uuid::Uuid::parse_str(id_raw).ok()?;
    Some(EventId::from_uuid(id))
}

pub(super) fn parse_phase_event_lines(
    raw_lines: &[&str],
    line_offset: usize,
) -> MethodologyResult<Vec<PhaseEventLine>> {
    let mut lines = Vec::new();
    for (idx, line) in raw_lines.iter().enumerate() {
        let line_number = line_offset + idx + 1;
        let parsed = serde_json::from_str::<PhaseEventLine>(line).map_err(|err| {
            MethodologyError::FieldValidation {
                field_path: format!("/phase-events.jsonl:{line_number}"),
                expected: "valid phase-event JSON envelope".into(),
                actual: err.to_string(),
                remediation:
                    "run `tanren-cli methodology reconcile-phase-events` to repair projection artifacts"
                        .into(),
            }
        })?;
        if parsed.schema_version != PHASE_EVENT_LINE_SCHEMA_VERSION {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/phase-events.jsonl:{line_number}/schema_version"),
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
