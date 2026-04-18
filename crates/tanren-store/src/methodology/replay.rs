//! JSONL replay — ingest a phase-events.jsonl file into the event log.
//!
//! Each line is one canonical `phase-events.jsonl` envelope matching
//! `docs/architecture/agent-tool-surface.md` §6. Replay validates:
//!
//! - line shape + typed payload decode,
//! - line/payload `spec_id` consistency,
//! - line `tool` consistency with payload variant,
//! - task-transition legality on the current store state, and
//! - idempotency by `event_id` dedupe.
//!
//! This module owns parse + validated apply only; the actual
//! phase-events projection write path remains in app-services.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tanren_domain::events::{DomainEvent, EventEnvelope, SCHEMA_VERSION};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::{RequiredGuard, TaskStatus, TaskTransitionKind};
use tanren_domain::{EntityRef, EventId, SpecId, TaskId};

use crate::Store;
use crate::entity::events;
use crate::errors::StoreError;
use crate::methodology::projections;

/// Result statistics returned by [`ingest_phase_events`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplayStats {
    pub lines_read: usize,
    pub events_appended: usize,
    pub events_skipped_duplicate_event_id: usize,
}

/// Typed error returned when replay fails.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed JSONL at {path}:{line}: {reason}\nraw: {raw}")]
    MalformedLine {
        path: PathBuf,
        line: usize,
        reason: String,
        raw: String,
    },
    #[error("spec_id mismatch at {path}:{line}: line={line_spec_id}, payload={payload_spec_id}")]
    SpecIdMismatch {
        path: PathBuf,
        line: usize,
        line_spec_id: SpecId,
        payload_spec_id: SpecId,
    },
    #[error("payload missing spec_id at {path}:{line}")]
    MissingPayloadSpecId { path: PathBuf, line: usize },
    #[error("tool mismatch at {path}:{line}: expected `{expected}`, got `{actual}`")]
    ToolMismatch {
        path: PathBuf,
        line: usize,
        expected: String,
        actual: String,
    },
    #[error("invalid task transition at {path}:{line}: task={task_id} {from} -> {attempted}")]
    InvalidTaskTransition {
        path: PathBuf,
        line: usize,
        task_id: TaskId,
        from: String,
        attempted: String,
    },
    #[error("task event without TaskCreated at {path}:{line}: task={task_id}")]
    MissingTaskCreate {
        path: PathBuf,
        line: usize,
        task_id: TaskId,
    },
    #[error("duplicate TaskCreated at {path}:{line}: task={task_id}")]
    DuplicateTaskCreate {
        path: PathBuf,
        line: usize,
        task_id: TaskId,
    },
    #[error("TaskCompleted without satisfied required guards at {path}:{line}: task={task_id}")]
    TaskCompletedMissingGuards {
        path: PathBuf,
        line: usize,
        task_id: TaskId,
    },
    #[error("store error: {source}")]
    Store {
        #[from]
        source: StoreError,
    },
}

/// Canonical `phase-events.jsonl` line envelope.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PhaseEventLine {
    event_id: EventId,
    spec_id: SpecId,
    phase: String,
    agent_session_id: String,
    timestamp: DateTime<Utc>,
    tool: String,
    payload: MethodologyEvent,
}

/// Ingest a canonical `phase-events.jsonl` file into the store.
///
/// The operation is not transactional across lines — a mid-file
/// failure leaves already-appended events in the store. Replay is
/// idempotent by event-id dedupe.
///
/// # Errors
/// See [`ReplayError`].
pub async fn ingest_phase_events(
    store: &Store,
    path: &Path,
    required_guards: &[RequiredGuard],
) -> Result<ReplayStats, ReplayError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| ReplayError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let text = String::from_utf8(bytes).map_err(|e| ReplayError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;

    let mut stats = ReplayStats::default();
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        stats.lines_read += 1;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: PhaseEventLine =
            serde_json::from_str(line).map_err(|source| ReplayError::MalformedLine {
                path: path.to_path_buf(),
                line: line_no,
                reason: source.to_string(),
                raw: line.to_owned(),
            })?;

        if event_id_exists(store, parsed.event_id).await? {
            stats.events_skipped_duplicate_event_id += 1;
            continue;
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
        let expected_tool = tool_name(&parsed.payload);
        if parsed.tool != expected_tool {
            return Err(ReplayError::ToolMismatch {
                path: path.to_path_buf(),
                line: line_no,
                expected: expected_tool.to_owned(),
                actual: parsed.tool,
            });
        }
        validate_task_transition(
            store,
            &parsed.payload,
            parsed.spec_id,
            line_no,
            path,
            required_guards,
        )
        .await?;
        let envelope = replay_envelope(parsed);
        store.append_methodology_event(&envelope).await?;
        stats.events_appended += 1;
    }
    Ok(stats)
}

fn replay_envelope(line: PhaseEventLine) -> EventEnvelope {
    let entity_ref = line.payload.entity_root();
    let payload = DomainEvent::Methodology {
        event: line.payload,
    };
    EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id: line.event_id,
        timestamp: line.timestamp,
        entity_ref,
        payload,
    }
}

async fn event_id_exists(store: &Store, event_id: EventId) -> Result<bool, ReplayError> {
    let row = events::Entity::find()
        .filter(events::Column::EventId.eq(event_id.into_uuid()))
        .one(store.conn())
        .await
        .map_err(StoreError::from)
        .map_err(|source| ReplayError::Store { source })?;
    Ok(row.is_some())
}

async fn validate_task_transition(
    store: &Store,
    event: &MethodologyEvent,
    spec_id: SpecId,
    line_no: usize,
    path: &Path,
    required_guards: &[RequiredGuard],
) -> Result<(), ReplayError> {
    let Some((task_id, kind)) = task_transition_kind(event) else {
        return Ok(());
    };
    let existing = projections::load_methodology_events_for_entity(
        store,
        EntityRef::Task(task_id),
        Some(spec_id),
        1_000,
    )
    .await
    .map_err(|source| match source {
        projections::MethodologyEventFetchError::Store { source } => ReplayError::Store { source },
    })?;

    let has_created = existing.iter().any(|ev| {
        matches!(
            ev,
            MethodologyEvent::TaskCreated(e) if e.task.id == task_id
        )
    });
    match event {
        MethodologyEvent::TaskCreated(_) => {
            if has_created {
                return Err(ReplayError::DuplicateTaskCreate {
                    path: path.to_path_buf(),
                    line: line_no,
                    task_id,
                });
            }
            return Ok(());
        }
        _ => {
            if !has_created {
                return Err(ReplayError::MissingTaskCreate {
                    path: path.to_path_buf(),
                    line: line_no,
                    task_id,
                });
            }
        }
    }

    let current = tanren_domain::methodology::events::fold_task_status(
        task_id,
        required_guards,
        existing.iter(),
    )
    .unwrap_or(TaskStatus::Pending);
    if matches!(event, MethodologyEvent::TaskCompleted(_))
        && !matches!(
            current,
            TaskStatus::Implemented { ref guards } if guards.satisfies(required_guards)
        )
    {
        return Err(ReplayError::TaskCompletedMissingGuards {
            path: path.to_path_buf(),
            line: line_no,
            task_id,
        });
    }
    current
        .legal_next(kind)
        .map_err(|e| ReplayError::InvalidTaskTransition {
            path: path.to_path_buf(),
            line: line_no,
            task_id,
            from: e.from.to_owned(),
            attempted: e.attempted.to_owned(),
        })?;
    Ok(())
}

fn task_transition_kind(event: &MethodologyEvent) -> Option<(TaskId, TaskTransitionKind)> {
    match event {
        MethodologyEvent::TaskCreated(e) => Some((e.task.id, TaskTransitionKind::Start)),
        MethodologyEvent::TaskStarted(e) => Some((e.task_id, TaskTransitionKind::Start)),
        MethodologyEvent::TaskImplemented(e) => Some((e.task_id, TaskTransitionKind::Implement)),
        MethodologyEvent::TaskGateChecked(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskAudited(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskAdherent(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskXChecked(e) => Some((e.task_id, TaskTransitionKind::Guard)),
        MethodologyEvent::TaskCompleted(e) => Some((e.task_id, TaskTransitionKind::Complete)),
        MethodologyEvent::TaskRevised(e) => Some((e.task_id, TaskTransitionKind::Revise)),
        MethodologyEvent::TaskAbandoned(e) => Some((e.task_id, TaskTransitionKind::Abandon)),
        _ => None,
    }
}

const fn tool_name(event: &MethodologyEvent) -> &'static str {
    match event {
        MethodologyEvent::SpecDefined(_) => "shape-spec",
        MethodologyEvent::TaskCreated(_) => "create_task",
        MethodologyEvent::TaskStarted(_) => "start_task",
        MethodologyEvent::TaskImplemented(_) => "complete_task",
        MethodologyEvent::TaskGateChecked(_)
        | MethodologyEvent::TaskAudited(_)
        | MethodologyEvent::TaskAdherent(_)
        | MethodologyEvent::TaskXChecked(_) => "<guard-phase>",
        MethodologyEvent::TaskCompleted(_) => "<orchestrator>",
        MethodologyEvent::TaskAbandoned(_) => "abandon_task",
        MethodologyEvent::TaskRevised(_) => "revise_task",
        MethodologyEvent::FindingAdded(_) => "add_finding",
        MethodologyEvent::AdherenceFindingAdded(_) => "record_adherence_finding",
        MethodologyEvent::RubricScoreRecorded(_) => "record_rubric_score",
        MethodologyEvent::NonNegotiableComplianceRecorded(_) => "record_non_negotiable_compliance",
        MethodologyEvent::SignpostAdded(_) => "add_signpost",
        MethodologyEvent::SignpostStatusUpdated(_) => "update_signpost_status",
        MethodologyEvent::IssueCreated(_) => "create_issue",
        MethodologyEvent::PhaseOutcomeReported(_) => "report_phase_outcome",
        MethodologyEvent::ReplyDirectiveRecorded(_) => "post_reply_directive",
        MethodologyEvent::SpecFrontmatterUpdated(_) => "spec.frontmatter",
        MethodologyEvent::DemoFrontmatterUpdated(_) => "demo.frontmatter",
        MethodologyEvent::UnauthorizedArtifactEdit(_) => "<enforcement>",
        MethodologyEvent::EvidenceSchemaError(_) => "<postflight>",
    }
}
