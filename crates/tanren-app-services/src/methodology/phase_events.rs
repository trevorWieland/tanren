//! Pure store → `phase-events.jsonl` projector.
//!
//! Provides both pure projection helpers (`line_for_envelope`,
//! `render_jsonl`) and append helpers used by the outbox projector.
//! The store is the source of truth; JSONL is a derived projection.
//! `tanren ingest-phase-events` is the inverse direction (JSONL → store).

use std::io::Write as _;
use std::path::Path;

use chrono::{DateTime, Utc};
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(encoded.as_bytes())
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(b"\n")
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
    Ok(())
}

/// Check whether `phase-events.jsonl` already contains `event_id`.
///
/// # Errors
/// Returns [`MethodologyError::Io`] when reading the file fails.
pub fn jsonl_contains_event_id(path: &Path, event_id: EventId) -> Result<bool, MethodologyError> {
    if !path.exists() {
        return Ok(false);
    }
    let needle = event_id.to_string();
    let file = std::fs::File::open(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
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
}
