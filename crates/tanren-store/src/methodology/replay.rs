//! JSONL replay for `phase-events.jsonl`.
//! Validates envelope shape, provenance/tool/spec consistency, and task-transition legality,
//! then atomically appends a deduped staged set to the event log.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use tanren_domain::events::{DomainEvent, EventEnvelope, SCHEMA_VERSION};
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::task::RequiredGuard;
use tanren_domain::{EventId, SpecId, TaskId};

use crate::Store;
use crate::converters::events as event_converters;
use crate::entity::events;
use crate::errors::StoreError;
use tokio::io::AsyncBufReadExt;

use super::replay_line_validation::{
    prefetch_task_specs_for_replay, validate_envelope_metadata, validate_event_semantics,
};
use super::replay_task_state::TaskValidationState;

const EVENT_ID_LOOKUP_BATCH_SIZE: usize = 256;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplayStats {
    pub lines_read: usize,
    pub events_appended: usize,
    pub events_skipped_duplicate_event_id: usize,
}

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
    #[error("origin_kind mismatch at {path}:{line}: expected `{expected}`, got `{actual}`")]
    OriginKindMismatch {
        path: PathBuf,
        line: usize,
        expected: String,
        actual: String,
    },
    #[error("missing origin_kind at {path}:{line}")]
    MissingOriginKind { path: PathBuf, line: usize },
    #[error("missing caused_by_tool_call_id for origin `{origin}` at {path}:{line}")]
    MissingCausedByToolCall {
        path: PathBuf,
        line: usize,
        origin: String,
    },
    #[error("validation failed: {details}")]
    FieldValidation { details: Box<ReplayFieldValidation> },
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

#[derive(Debug)]
pub struct ReplayFieldValidation {
    pub path: PathBuf,
    pub line: usize,
    pub field_path: String,
    pub expected: String,
    pub actual: String,
    pub remediation: String,
}

impl std::fmt::Display for ReplayFieldValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{} {} expected {}, got {}",
            self.path.display(),
            self.line,
            self.field_path,
            self.expected,
            self.actual
        )
    }
}

impl ReplayError {
    pub(super) fn field_validation(
        path: PathBuf,
        line: usize,
        field_path: String,
        expected: String,
        actual: String,
        remediation: String,
    ) -> Self {
        Self::FieldValidation {
            details: Box::new(ReplayFieldValidation {
                path,
                line,
                field_path,
                expected,
                actual,
                remediation,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct PhaseEventLine {
    pub(super) event_id: EventId,
    pub(super) spec_id: SpecId,
    pub(super) phase: PhaseId,
    pub(super) agent_session_id: String,
    pub(super) timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) caused_by_tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) origin_kind: Option<PhaseEventOriginKind>,
    pub(super) tool: String,
    pub(super) payload: MethodologyEvent,
}

#[derive(Debug)]
struct ParsedLine {
    line_no: usize,
    line: PhaseEventLine,
}

#[derive(Debug, Default)]
pub(super) struct IngestState {
    staged: Vec<EventEnvelope>,
    seen_event_ids: HashSet<EventId>,
    pub task_state: TaskValidationState,
    pub task_spec_lookup: HashMap<TaskId, Option<SpecId>>,
}

/// Ingest `phase-events.jsonl` into the store with strict replay defaults.
pub async fn ingest_phase_events(
    store: &Store,
    path: &Path,
    required_guards: &[RequiredGuard],
) -> Result<ReplayStats, ReplayError> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|source| ReplayError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut reader = tokio::io::BufReader::new(file);
    let mut stats = ReplayStats::default();
    let mut ingest_state = IngestState::default();
    let mut pending: Vec<ParsedLine> = Vec::with_capacity(EVENT_ID_LOOKUP_BATCH_SIZE);
    let mut line_buf = String::new();

    loop {
        line_buf.clear();
        let read = reader
            .read_line(&mut line_buf)
            .await
            .map_err(|source| ReplayError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        stats.lines_read += 1;
        let line_no = stats.lines_read;
        let line = line_buf.trim_end_matches(['\r', '\n']);
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
        pending.push(ParsedLine {
            line_no,
            line: parsed,
        });
        if pending.len() >= EVENT_ID_LOOKUP_BATCH_SIZE {
            process_pending_chunk(
                store,
                path,
                required_guards,
                &mut pending,
                &mut ingest_state,
                &mut stats,
            )
            .await?;
        }
    }
    if !pending.is_empty() {
        process_pending_chunk(
            store,
            path,
            required_guards,
            &mut pending,
            &mut ingest_state,
            &mut stats,
        )
        .await?;
    }
    append_staged_atomic(store, &ingest_state.staged).await?;
    Ok(stats)
}

async fn process_pending_chunk(
    store: &Store,
    path: &Path,
    required_guards: &[RequiredGuard],
    pending: &mut Vec<ParsedLine>,
    ingest_state: &mut IngestState,
    stats: &mut ReplayStats,
) -> Result<(), ReplayError> {
    prefetch_task_specs_for_replay(store, &collect_attached_task_ids(pending), ingest_state)
        .await?;
    let ids: Vec<EventId> = pending
        .iter()
        .map(|entry| entry.line.event_id)
        .filter(|event_id| !ingest_state.seen_event_ids.contains(event_id))
        .collect();
    let existing_ids = existing_event_ids_for_batch(store, &ids).await?;
    for entry in pending.drain(..) {
        let event_id = entry.line.event_id;
        if ingest_state.seen_event_ids.contains(&event_id) || existing_ids.contains(&event_id) {
            ingest_state.seen_event_ids.insert(event_id);
            stats.events_skipped_duplicate_event_id += 1;
            continue;
        }
        validate_envelope_metadata(path, entry.line_no, &entry.line)?;
        validate_event_semantics(
            store,
            path,
            entry.line_no,
            &entry.line,
            required_guards,
            ingest_state,
        )
        .await?;
        let envelope = replay_envelope(entry.line);
        ingest_state.seen_event_ids.insert(envelope.event_id);
        ingest_state.staged.push(envelope);
        stats.events_appended += 1;
    }
    Ok(())
}

fn collect_attached_task_ids(pending: &[ParsedLine]) -> HashSet<TaskId> {
    let mut out = HashSet::new();
    for entry in pending {
        match &entry.line.payload {
            MethodologyEvent::FindingAdded(e) => {
                if let Some(task_id) = e.finding.attached_task {
                    out.insert(task_id);
                }
            }
            MethodologyEvent::AdherenceFindingAdded(e) => {
                if let Some(task_id) = e.finding.attached_task {
                    out.insert(task_id);
                }
            }
            _ => {}
        }
    }
    out
}

fn replay_envelope(line: PhaseEventLine) -> EventEnvelope {
    let payload = line.payload;
    EventEnvelope {
        schema_version: SCHEMA_VERSION,
        event_id: line.event_id,
        timestamp: line.timestamp,
        entity_ref: payload.entity_root(),
        payload: DomainEvent::Methodology { event: payload },
    }
}

async fn existing_event_ids_for_batch(
    store: &Store,
    event_ids: &[EventId],
) -> Result<HashSet<EventId>, ReplayError> {
    if event_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let ids: Vec<uuid::Uuid> = event_ids
        .iter()
        .map(|event_id| event_id.into_uuid())
        .collect();
    let rows = events::Entity::find()
        .filter(events::Column::EventId.is_in(ids))
        .all(store.conn())
        .await
        .map_err(StoreError::from)
        .map_err(|source| ReplayError::Store { source })?;
    let mut out = HashSet::with_capacity(rows.len());
    for row in rows {
        out.insert(EventId::from_uuid(row.event_id));
    }
    Ok(out)
}

async fn append_staged_atomic(store: &Store, staged: &[EventEnvelope]) -> Result<(), ReplayError> {
    if staged.is_empty() {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(staged.len());
    let mut methodology_events = Vec::with_capacity(staged.len());
    for envelope in staged {
        rows.push(event_converters::envelope_to_active_model(envelope)?);
        if let DomainEvent::Methodology { event } = &envelope.payload {
            methodology_events.push(event.clone());
        }
    }
    store
        .conn()
        .transaction::<_, (), StoreError>(move |txn| {
            Box::pin(async move {
                for (idx, row) in rows.into_iter().enumerate() {
                    events::Entity::insert(row).exec(txn).await?;
                    if let Some(event) = methodology_events.get(idx) {
                        super::task_status_projection::upsert_task_status_projection_txn(
                            txn, event,
                        )
                        .await?;
                    }
                }
                Ok(())
            })
        })
        .await
        .map_err(|source| ReplayError::Store {
            source: source.into(),
        })?;
    Ok(())
}
