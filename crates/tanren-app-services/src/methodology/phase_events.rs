//! Pure store → `phase-events.jsonl` projector.
//!
//! Provides both pure projection helpers (`line_for_envelope`,
//! `render_jsonl`) and append helpers used by the outbox projector.
//! The store is the source of truth; JSONL is a derived projection.
//! `tanren ingest-phase-events` is the inverse direction (JSONL → store).

use std::io::Write as _;
use std::num::NonZeroU32;
use std::path::Path;

use chrono::{DateTime, Utc};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use tanren_domain::events::EventEnvelope;
use tanren_domain::methodology::event_tool::{PhaseEventOriginKind, canonical_tool_for_event};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::{EventId, SpecId};

use super::errors::MethodologyError;
#[path = "phase_events_storage.rs"]
mod phase_events_storage;
use self::phase_events_storage::{
    compact_jsonl_event_log as compact_jsonl_event_log_impl, event_id_marker_exists,
    jsonl_contains_event_id_locked, phase_events_lock_path, should_sync_after_append,
    upsert_event_id_marker,
};

/// Schema version for `phase-events.jsonl` line envelopes.
pub const PHASE_EVENT_LINE_SCHEMA_VERSION: &str = "1.0.0";
const FSYNC_STATE_SCHEMA_VERSION: &str = "1.0.0";

/// Append durability policy for `phase-events.jsonl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseEventsAppendPolicy {
    /// Force `sync_all` every N successful appends.
    pub fsync_every: NonZeroU32,
}

impl Default for PhaseEventsAppendPolicy {
    fn default() -> Self {
        let parsed = std::env::var("TANREN_PHASE_EVENTS_FSYNC_EVERY")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .and_then(NonZeroU32::new)
            .unwrap_or_else(|| NonZeroU32::new(1).expect("non-zero literal"));
        Self {
            fsync_every: parsed,
        }
    }
}

impl PhaseEventsAppendPolicy {
    /// Build a policy from a configured sync interval.
    ///
    /// Returns `None` when `fsync_every` is zero.
    #[must_use]
    pub fn from_fsync_every(fsync_every: u32) -> Option<Self> {
        NonZeroU32::new(fsync_every).map(|non_zero| Self {
            fsync_every: non_zero,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PhaseEventsFsyncState {
    schema_version: String,
    pending_since_last_sync: u32,
}

/// Compaction result for `phase-events.jsonl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseEventsCompactionReport {
    pub total_lines_before: u64,
    pub total_lines_after: u64,
    pub duplicates_removed: u64,
    pub empty_lines_removed: u64,
    pub rewrote_file: bool,
}

/// Canonical `phase-events.jsonl` line envelope per
/// `docs/architecture/agent-tool-surface.md` §6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseEventLine {
    pub schema_version: String,
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
                schema_version: PHASE_EVENT_LINE_SCHEMA_VERSION.to_owned(),
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
        schema_version: PHASE_EVENT_LINE_SCHEMA_VERSION.to_owned(),
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
    let _ = append_jsonl_encoded_line_if_missing_event_id_with_policy(
        path,
        encoded,
        None,
        PhaseEventsAppendPolicy::default(),
    )?;
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
    append_jsonl_encoded_line_if_missing_event_id_with_policy(
        path,
        encoded,
        event_id,
        PhaseEventsAppendPolicy::default(),
    )
}

/// Append one serialized JSON line unless `event_id` already exists.
///
/// This variant allows callers to configure append-fsync batching.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on filesystem failures.
pub fn append_jsonl_encoded_line_if_missing_event_id_with_policy(
    path: &Path,
    encoded: &str,
    event_id: Option<EventId>,
    policy: PhaseEventsAppendPolicy,
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
    if let Some(event_id) = event_id {
        if event_id_marker_exists(path, event_id) {
            drop(lock_file);
            return Ok(false);
        }
        if jsonl_contains_event_id_locked(path, event_id)? {
            upsert_event_id_marker(path, event_id)?;
            drop(lock_file);
            return Ok(false);
        }
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
    if let Some(event_id) = event_id {
        upsert_event_id_marker(path, event_id)?;
    }
    if should_sync_after_append(path, policy)? {
        file.sync_all().map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if let Some(parent) = path.parent()
            && let Ok(dir) = std::fs::File::open(parent)
        {
            let _ = dir.sync_all();
        }
    }
    drop(lock_file);
    Ok(true)
}

/// Compact a `phase-events.jsonl` projection file.
///
/// Compaction is deterministic and preserves first-seen order:
/// - removes blank lines
/// - removes duplicate `event_id` entries (keeps first occurrence)
/// - rewrites line terminators to canonical LF
/// - rebuilds the event-id marker index sidecar
///
/// # Errors
/// Returns [`MethodologyError::Io`] on filesystem failures.
pub fn compact_jsonl_event_log(
    path: &Path,
) -> Result<PhaseEventsCompactionReport, MethodologyError> {
    compact_jsonl_event_log_impl(path)
}

/// Check whether `phase-events.jsonl` already contains `event_id`.
///
/// # Errors
/// Returns [`MethodologyError::Io`] when reading the file fails.
pub fn jsonl_contains_event_id(path: &Path, event_id: EventId) -> Result<bool, MethodologyError> {
    if event_id_marker_exists(path, event_id) {
        return Ok(true);
    }
    if !path.exists() {
        return Ok(false);
    }
    let found = jsonl_contains_event_id_locked(path, event_id)?;
    if found {
        let _ = upsert_event_id_marker(path, event_id);
    }
    Ok(found)
}
