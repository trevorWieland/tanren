use std::path::Path;

use tanren_domain::methodology::event_tool::{
    PhaseEventOriginKind, canonical_tool_for_event, is_tool_allowed_for_event,
};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::RequiredGuard;
use tanren_domain::methodology::validation::{
    ValidationIssue, validate_finding_attached_task_spec, validate_finding_line_numbers,
    validate_task_abandon_semantics,
};
use tanren_domain::{EntityRef, SpecId, TaskId};

use crate::Store;
use crate::methodology::projections;

use super::replay::{IngestState, PhaseEventLine, ReplayError, ReplayOptions};
use super::replay_task_state::validate_task_transition;

const TASK_EVENT_PAGE_SIZE: u64 = 1_000;

pub(super) fn validate_envelope_metadata(
    path: &Path,
    line_no: usize,
    parsed: &PhaseEventLine,
    options: ReplayOptions,
) -> Result<(), ReplayError> {
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
    if !is_tool_allowed_for_event(&parsed.payload, &parsed.tool) {
        return Err(ReplayError::ToolMismatch {
            path: path.to_path_buf(),
            line: line_no,
            expected: expected_tool.to_owned(),
            actual: parsed.tool.clone(),
        });
    }
    validate_origin_metadata(path, line_no, parsed, options)
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

async fn validate_attached_task_for_finding(
    store: &Store,
    path: &Path,
    line_no: usize,
    finding_spec_id: SpecId,
    attached_task: TaskId,
    ingest_state: &mut IngestState,
) -> Result<(), ReplayError> {
    let resolved_spec_id = resolve_task_spec_for_replay(store, attached_task, ingest_state).await?;
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
    ingest_state: &mut IngestState,
) -> Result<Option<SpecId>, ReplayError> {
    if let Some(cached) = ingest_state.task_spec_lookup.get(&task_id) {
        return Ok(*cached);
    }
    let events = projections::load_methodology_events_for_entity(
        store,
        EntityRef::Task(task_id),
        None,
        TASK_EVENT_PAGE_SIZE,
    )
    .await
    .map_err(|source| match source {
        projections::MethodologyEventFetchError::Store { source } => ReplayError::Store { source },
    })?;
    let resolved = events.into_iter().find_map(|event| match event {
        MethodologyEvent::TaskCreated(e) => Some(e.task.spec_id),
        _ => None,
    });
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
    options: ReplayOptions,
) -> Result<(), ReplayError> {
    let Some(origin_kind) = parsed.origin_kind else {
        if options.allow_legacy_provenance && parsed.caused_by_tool_call_id.is_none() {
            return Ok(());
        }
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
