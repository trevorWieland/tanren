use std::path::Path;

use tanren_domain::methodology::event_tool::{PhaseEventOriginKind, canonical_tool_for_event};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::RequiredGuard;
use tanren_domain::methodology::validation::{
    ValidationIssue, validate_finding_attached_task_spec, validate_finding_line_numbers,
    validate_task_abandon_semantics,
};
use tanren_domain::{SpecId, TaskId};

use crate::Store;
use crate::methodology::projections;

use super::replay::{IngestState, PhaseEventLine, ReplayError};
use super::replay_task_state::validate_task_transition;

const PHASE_EVENT_LINE_SCHEMA_VERSION: &str = "1.0.0";

pub(super) fn validate_envelope_metadata(
    path: &Path,
    line_no: usize,
    parsed: &PhaseEventLine,
) -> Result<(), ReplayError> {
    if parsed.schema_version != PHASE_EVENT_LINE_SCHEMA_VERSION {
        return Err(ReplayError::field_validation(
            path.to_path_buf(),
            line_no,
            "/schema_version".into(),
            PHASE_EVENT_LINE_SCHEMA_VERSION.into(),
            parsed.schema_version.clone(),
            "upgrade or regenerate phase-events.jsonl to the current line schema".into(),
        ));
    }
    let payload_spec_id =
        parsed
            .payload
            .spec_id()
            .ok_or_else(|| ReplayError::MissingPayloadSpecId {
                path: path.to_path_buf(),
                line: line_no,
            })?;
    if payload_spec_id != parsed.spec_id {
        return Err(ReplayError::SpecIdMismatch {
            path: path.to_path_buf(),
            line: line_no,
            line_spec_id: parsed.spec_id,
            payload_spec_id,
        });
    }
    let expected_tool = canonical_tool_for_event(&parsed.payload);
    if parsed.tool != expected_tool {
        return Err(ReplayError::ToolMismatch {
            path: path.to_path_buf(),
            line: line_no,
            expected: expected_tool.to_owned(),
            actual: parsed.tool.clone(),
        });
    }
    validate_origin_metadata(path, line_no, parsed)
}

pub(super) async fn validate_event_semantics(
    store: &Store,
    path: &Path,
    line_no: usize,
    parsed: &PhaseEventLine,
    required_guards: &[RequiredGuard],
    ingest_state: &mut IngestState,
) -> Result<(), ReplayError> {
    match &parsed.payload {
        MethodologyEvent::TaskAbandoned(e) => {
            validate_task_abandon_semantics(
                &parsed.phase,
                e.disposition,
                &e.replacements,
                &e.explicit_user_discard_provenance,
            )
            .map_err(|issue| replay_validation_issue(path, line_no, issue))?;
        }
        MethodologyEvent::FindingAdded(e) => {
            validate_finding_line_numbers(&e.finding.line_numbers)
                .map_err(|issue| replay_validation_issue(path, line_no, issue))?;
            if let Some(attached_task) = e.finding.attached_task {
                validate_attached_task_for_finding(
                    store,
                    path,
                    line_no,
                    parsed.spec_id,
                    attached_task,
                    ingest_state,
                )
                .await?;
            }
        }
        MethodologyEvent::AdherenceFindingAdded(e) => {
            validate_finding_line_numbers(&e.finding.line_numbers)
                .map_err(|issue| replay_validation_issue(path, line_no, issue))?;
            if let Some(attached_task) = e.finding.attached_task {
                validate_attached_task_for_finding(
                    store,
                    path,
                    line_no,
                    parsed.spec_id,
                    attached_task,
                    ingest_state,
                )
                .await?;
            }
        }
        _ => {}
    }
    validate_task_transition(
        store,
        &parsed.payload,
        parsed.spec_id,
        line_no,
        path,
        required_guards,
        &mut ingest_state.task_state,
    )
    .await
}

pub(super) async fn prefetch_task_specs_for_replay(
    store: &Store,
    task_ids: &std::collections::HashSet<TaskId>,
    ingest_state: &mut IngestState,
) -> Result<(), ReplayError> {
    if task_ids.is_empty() {
        return Ok(());
    }
    let unresolved: Vec<TaskId> = task_ids
        .iter()
        .copied()
        .filter(|task_id| !ingest_state.task_spec_lookup.contains_key(task_id))
        .collect();
    if unresolved.is_empty() {
        return Ok(());
    }

    let projection = store
        .load_methodology_task_specs_projection(&unresolved)
        .await
        .map_err(|source| ReplayError::Store { source })?;
    let missing: Vec<TaskId> = unresolved
        .iter()
        .copied()
        .filter(|task_id| !projection.contains_key(task_id))
        .collect();
    let fallback = projections::task_specs_by_ids(store, &missing)
        .await
        .map_err(|source| match source {
            projections::MethodologyEventFetchError::Store { source } => {
                ReplayError::Store { source }
            }
        })?;

    for task_id in unresolved {
        let resolved = projection
            .get(&task_id)
            .copied()
            .or_else(|| fallback.get(&task_id).copied());
        ingest_state.task_spec_lookup.insert(task_id, resolved);
    }
    Ok(())
}

async fn validate_attached_task_for_finding(
    store: &Store,
    path: &Path,
    line_no: usize,
    finding_spec_id: SpecId,
    attached_task: TaskId,
    ingest_state: &mut IngestState,
) -> Result<(), ReplayError> {
    let resolved_spec_id =
        resolve_task_spec_for_replay(store, attached_task, finding_spec_id, ingest_state).await?;
    let Some(resolved_spec_id) = resolved_spec_id else {
        return Err(ReplayError::field_validation(
            path.to_path_buf(),
            line_no,
            "/attached_task".into(),
            "existing task id".into(),
            attached_task.to_string(),
            "attach a task that already exists in the event store".into(),
        ));
    };
    validate_finding_attached_task_spec(attached_task, finding_spec_id, resolved_spec_id)
        .map_err(|issue| replay_validation_issue(path, line_no, issue))
}

async fn resolve_task_spec_for_replay(
    store: &Store,
    task_id: TaskId,
    finding_spec_id: SpecId,
    ingest_state: &mut IngestState,
) -> Result<Option<SpecId>, ReplayError> {
    // During fresh replay, task creations from earlier lines in the same ingest
    // batch are not yet persisted to projections. Honor in-flight replay state.
    if ingest_state
        .task_state
        .by_task
        .get(&task_id)
        .is_some_and(|state| state.has_created)
    {
        ingest_state
            .task_spec_lookup
            .entry(task_id)
            .or_insert(Some(finding_spec_id));
        return Ok(Some(finding_spec_id));
    }
    if let Some(cached) = ingest_state.task_spec_lookup.get(&task_id) {
        return Ok(*cached);
    }
    let projection = store
        .load_methodology_task_specs_projection(&[task_id])
        .await
        .map_err(|source| ReplayError::Store { source })?;
    let resolved = if let Some(spec_id) = projection.get(&task_id).copied() {
        Some(spec_id)
    } else {
        let fallback = projections::task_specs_by_ids(store, &[task_id])
            .await
            .map_err(|source| match source {
                projections::MethodologyEventFetchError::Store { source } => {
                    ReplayError::Store { source }
                }
            })?;
        fallback.get(&task_id).copied()
    };
    ingest_state.task_spec_lookup.insert(task_id, resolved);
    Ok(resolved)
}

fn replay_validation_issue(path: &Path, line_no: usize, issue: ValidationIssue) -> ReplayError {
    ReplayError::field_validation(
        path.to_path_buf(),
        line_no,
        issue.field_path,
        issue.expected,
        issue.actual,
        issue.remediation,
    )
}

fn validate_origin_metadata(
    path: &Path,
    line_no: usize,
    parsed: &PhaseEventLine,
) -> Result<(), ReplayError> {
    let Some(origin_kind) = parsed.origin_kind else {
        return Err(ReplayError::MissingOriginKind {
            path: path.to_path_buf(),
            line: line_no,
        });
    };
    let is_system_event = matches!(
        parsed.payload,
        MethodologyEvent::UnauthorizedArtifactEdit(_) | MethodologyEvent::EvidenceSchemaError(_)
    );
    if is_system_event && origin_kind != PhaseEventOriginKind::System {
        return Err(ReplayError::OriginKindMismatch {
            path: path.to_path_buf(),
            line: line_no,
            expected: "system".into(),
            actual: origin_kind_tag(origin_kind).into(),
        });
    }
    if !is_system_event && origin_kind == PhaseEventOriginKind::System {
        return Err(ReplayError::OriginKindMismatch {
            path: path.to_path_buf(),
            line: line_no,
            expected: "tool_primary|tool_derived".into(),
            actual: "system".into(),
        });
    }
    if origin_kind == PhaseEventOriginKind::ToolDerived && parsed.caused_by_tool_call_id.is_none() {
        return Err(ReplayError::MissingCausedByToolCall {
            path: path.to_path_buf(),
            line: line_no,
            origin: origin_kind_tag(origin_kind).into(),
        });
    }
    Ok(())
}

const fn origin_kind_tag(kind: PhaseEventOriginKind) -> &'static str {
    match kind {
        PhaseEventOriginKind::ToolPrimary => "tool_primary",
        PhaseEventOriginKind::ToolDerived => "tool_derived",
        PhaseEventOriginKind::System => "system",
    }
}
