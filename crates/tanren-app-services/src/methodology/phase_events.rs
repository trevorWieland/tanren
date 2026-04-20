//! Pure store → `phase-events.jsonl` projector.
//!
//! Provides both pure projection helpers (`line_for_envelope`,
//! `render_jsonl`) and append helpers used by the outbox projector.
//! The store is the source of truth; JSONL is a derived projection.
//! `tanren ingest-phase-events` is the inverse direction (JSONL → store).

use std::io::Write as _;
use std::path::Path;

use chrono::{DateTime, Utc};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use tanren_domain::events::EventEnvelope;
use tanren_domain::methodology::event_tool::{PhaseEventOriginKind, canonical_tool_for_event};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::{EventId, SpecId};

use super::errors::MethodologyError;

/// Canonical `phase-events.jsonl` line envelope per
/// `docs/architecture/agent-tool-surface.md` §6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseEventLine {
    pub event_id: EventId,
    pub spec_id: SpecId,
    pub phase: String,
    pub agent_session_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caused_by_tool_call_id: Option<String>,
    pub origin_kind: PhaseEventOriginKind,
    pub tool: String,
    pub payload: MethodologyEvent,
}

/// Optional projector attribution overrides supplied by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhaseEventAttribution {
    pub caused_by_tool_call_id: Option<String>,
    pub origin_kind: Option<PhaseEventOriginKind>,
    pub tool: Option<String>,
}

/// Project a slice of `EventEnvelope` rows into `PhaseEventLine`s.
///
/// Only methodology-typed events are projected; other variants are
/// silently skipped (the store-query caller is expected to pre-filter).
/// Each line records the session id of the phase that emitted the
/// event — callers derive this from a
/// [`PhaseOutcomeReported`](MethodologyEvent::PhaseOutcomeReported)
/// observation in the same stream or pass it in.
///
/// Deterministic over input ordering: emits lines in input order.
#[must_use]
pub fn project_phase_events(
    envelopes: &[EventEnvelope],
    spec_id: SpecId,
    phase: &str,
    agent_session_id: &str,
) -> Vec<PhaseEventLine> {
    envelopes
        .iter()
        .filter_map(|env| {
            let tanren_domain::events::DomainEvent::Methodology { event } = &env.payload else {
                return None;
            };
            if event.spec_id() != Some(spec_id) {
                return None;
            }
            Some(PhaseEventLine {
                event_id: env.event_id,
                spec_id,
                phase: phase.to_owned(),
                agent_session_id: agent_session_id.to_owned(),
                timestamp: env.timestamp,
                caused_by_tool_call_id: None,
                origin_kind: PhaseEventOriginKind::default_for_event(event),
                tool: canonical_tool_for_event(event).to_owned(),
                payload: event.clone(),
            })
        })
        .collect()
}

/// Render `PhaseEventLine`s to canonical JSONL text (LF terminator per
/// line). Callers write the result via atomic tempfile+rename.
///
/// # Errors
/// Returns [`MethodologyError::Internal`] if JSON serialization fails
/// (only possible for types that have custom serializers which reject
/// their own value — should not occur for the methodology shapes).
pub fn render_jsonl(lines: &[PhaseEventLine]) -> Result<String, MethodologyError> {
    let mut out = String::new();
    for line in lines {
        let j =
            serde_json::to_string(line).map_err(|e| MethodologyError::Internal(e.to_string()))?;
        out.push_str(&j);
        out.push('\n');
    }
    Ok(out)
}

/// Build one [`PhaseEventLine`] from one envelope if (and only if) it
/// is a methodology event correlated to `spec_id`.
#[must_use]
pub fn line_for_envelope(
    envelope: &EventEnvelope,
    spec_id: SpecId,
    phase: &str,
    agent_session_id: &str,
) -> Option<PhaseEventLine> {
    line_for_envelope_with_attribution(
        envelope,
        spec_id,
        phase,
        agent_session_id,
        &PhaseEventAttribution::default(),
    )
}

/// Build one [`PhaseEventLine`] from one envelope, allowing the caller to
/// override attribution fields.
#[must_use]
pub fn line_for_envelope_with_attribution(
    envelope: &EventEnvelope,
    spec_id: SpecId,
    phase: &str,
    agent_session_id: &str,
    attribution: &PhaseEventAttribution,
) -> Option<PhaseEventLine> {
    let tanren_domain::events::DomainEvent::Methodology { event } = &envelope.payload else {
        return None;
    };
    if event.spec_id() != Some(spec_id) {
        return None;
    }
    let default_origin = PhaseEventOriginKind::default_for_event(event);
    Some(PhaseEventLine {
        event_id: envelope.event_id,
        spec_id,
        phase: phase.to_owned(),
        agent_session_id: agent_session_id.to_owned(),
        timestamp: envelope.timestamp,
        caused_by_tool_call_id: attribution.caused_by_tool_call_id.clone(),
        origin_kind: attribution.origin_kind.unwrap_or(default_origin),
        tool: attribution
            .tool
            .clone()
            .unwrap_or_else(|| canonical_tool_for_event(event).to_owned()),
        payload: event.clone(),
    })
}

/// Append one line to `phase-events.jsonl` using append-only file I/O.
///
/// Writes one JSON line + LF, then fsyncs the file. Callers use the
/// outbox status marker as the durable exactly-once guard.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on filesystem failures and
/// [`MethodologyError::Internal`] on serialization failure.
pub fn append_jsonl_line_atomic(
    path: &Path,
    line: &PhaseEventLine,
) -> Result<(), MethodologyError> {
    let encoded =
        serde_json::to_string(line).map_err(|e| MethodologyError::Internal(e.to_string()))?;
    append_jsonl_encoded_line(path, &encoded)
}

/// Append one already-serialized JSON line to `phase-events.jsonl`.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on filesystem failures.
pub fn append_jsonl_encoded_line(path: &Path, encoded: &str) -> Result<(), MethodologyError> {
    let _ = append_jsonl_encoded_line_if_missing_event_id(path, encoded, None)?;
    Ok(())
}

/// Append one serialized JSON line unless `event_id` already exists.
///
/// When `event_id` is `Some`, this function checks for an existing
/// event id under the same file lock to prevent duplicate writes from
/// concurrent outbox drainers.
///
/// Returns `Ok(true)` when a line was appended and `Ok(false)` when
/// the line was skipped due to an existing event id.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on filesystem failures.
pub fn append_jsonl_encoded_line_if_missing_event_id(
    path: &Path,
    encoded: &str,
    event_id: Option<EventId>,
) -> Result<bool, MethodologyError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let lock_path = phase_events_lock_path(path);
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lock_path)
        .map_err(|source| MethodologyError::Io {
            path: lock_path.clone(),
            source,
        })?;
    lock_file
        .lock_exclusive()
        .map_err(|source| MethodologyError::Io {
            path: lock_path,
            source,
        })?;
    if let Some(event_id) = event_id
        && jsonl_contains_event_id_locked(path, event_id)?
    {
        drop(lock_file);
        return Ok(false);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut line = String::with_capacity(encoded.len() + 1);
    line.push_str(encoded);
    line.push('\n');
    file.write_all(line.as_bytes())
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.sync_all().map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some(parent) = path.parent()
        && let Ok(dir) = std::fs::File::open(parent)
    {
        let _ = dir.sync_all();
    }
    drop(lock_file);
    Ok(true)
}

fn phase_events_lock_path(path: &Path) -> std::path::PathBuf {
    let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return path.with_extension("lock");
    };
    path.with_file_name(format!("{file_name}.lock"))
}

/// Check whether `phase-events.jsonl` already contains `event_id`.
///
/// # Errors
/// Returns [`MethodologyError::Io`] when reading the file fails.
pub fn jsonl_contains_event_id(path: &Path, event_id: EventId) -> Result<bool, MethodologyError> {
    if !path.exists() {
        return Ok(false);
    }
    jsonl_contains_event_id_locked(path, event_id)
}

fn jsonl_contains_event_id_locked(
    path: &Path,
    event_id: EventId,
) -> Result<bool, MethodologyError> {
    if !path.exists() {
        return Ok(false);
    }
    let needle = event_id.to_string();
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(MethodologyError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let reader = std::io::BufReader::new(file);
    for line in std::io::BufRead::lines(reader) {
        let line = line.map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line)
            && value.get("event_id").and_then(serde_json::Value::as_str) == Some(needle.as_str())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanren_domain::NonEmptyString;
    use tanren_domain::events::{DomainEvent, EventEnvelope};
    use tanren_domain::methodology::events::{TaskCreated, TaskStarted};
    use tanren_domain::methodology::task::{Task, TaskOrigin, TaskStatus};
    use tanren_domain::{EntityRef, EventId, TaskId};

    fn task(spec: SpecId) -> Task {
        Task {
            id: TaskId::new(),
            spec_id: spec,
            title: NonEmptyString::try_new("t").expect("non-empty"),
            description: String::new(),
            acceptance_criteria: vec![],
            origin: TaskOrigin::ShapeSpec,
            status: TaskStatus::Pending,
            depends_on: vec![],
            parent_task_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn project_filters_by_spec_id() {
        let spec_a = SpecId::new();
        let spec_b = SpecId::new();
        let task_a = task(spec_a);
        let task_b = task(spec_b);
        let env_a = EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::new(),
            timestamp: Utc::now(),
            entity_ref: EntityRef::Task(task_a.id),
            payload: DomainEvent::Methodology {
                event: MethodologyEvent::TaskCreated(TaskCreated {
                    task: Box::new(task_a),
                    origin: TaskOrigin::ShapeSpec,
                    idempotency_key: None,
                }),
            },
        };
        let env_b = EventEnvelope {
            schema_version: tanren_domain::SCHEMA_VERSION,
            event_id: EventId::new(),
            timestamp: Utc::now(),
            entity_ref: EntityRef::Task(task_b.id),
            payload: DomainEvent::Methodology {
                event: MethodologyEvent::TaskStarted(TaskStarted {
                    task_id: task_b.id,
                    spec_id: spec_b,
                }),
            },
        };
        let lines = project_phase_events(&[env_a, env_b], spec_a, "do-task", "session-1");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spec_id, spec_a);
    }

    #[test]
    fn render_jsonl_is_lf_terminated() {
        let spec = SpecId::new();
        let line = PhaseEventLine {
            event_id: EventId::new(),
            spec_id: spec,
            phase: "do-task".into(),
            agent_session_id: "s1".into(),
            timestamp: Utc::now(),
            caused_by_tool_call_id: Some("call-1".into()),
            origin_kind: PhaseEventOriginKind::ToolPrimary,
            tool: "start_task".into(),
            payload: MethodologyEvent::TaskStarted(TaskStarted {
                task_id: TaskId::new(),
                spec_id: spec,
            }),
        };
        let text = render_jsonl(&[line.clone(), line]).expect("render");
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn append_jsonl_encoded_line_is_line_safe_under_concurrency() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("phase-events.jsonl");
        let workers = 8usize;
        let per_worker = 100usize;
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let path = path.clone();
            handles.push(std::thread::spawn(move || {
                for i in 0..per_worker {
                    let encoded = format!("{{\"worker\":{worker},\"seq\":{i}}}");
                    append_jsonl_encoded_line(&path, &encoded).expect("append");
                }
            }));
        }
        for handle in handles {
            handle.join().expect("join");
        }
        let text = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), workers * per_worker);
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("valid json");
            assert!(parsed.get("worker").is_some());
            assert!(parsed.get("seq").is_some());
        }
    }

    #[test]
    fn append_jsonl_encoded_line_dedup_skips_existing_event_id() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("phase-events.jsonl");
        let event_id = EventId::new();
        let encoded = format!("{{\"event_id\":\"{event_id}\",\"value\":1}}");
        let first = append_jsonl_encoded_line_if_missing_event_id(&path, &encoded, Some(event_id))
            .expect("first append");
        let second = append_jsonl_encoded_line_if_missing_event_id(&path, &encoded, Some(event_id))
            .expect("second append");
        assert!(first, "first append should write");
        assert!(!second, "second append should be skipped as duplicate");
        let raw = std::fs::read_to_string(path).expect("read");
        let lines: Vec<&str> = raw.lines().filter(|line| !line.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1);
    }
}
