//! JSONL replay — ingest a phase-events.jsonl file into the event log.
//!
//! Each line is one [`EventEnvelope`] with a methodology payload.
//! Malformed lines return a typed error carrying the line number and
//! raw content for triage. This module owns only the parse + dispatch;
//! the actual phase-events.jsonl **write** happens in
//! `tanren-app-services::methodology::phase_events` at the phase
//! boundary (store is the single source of truth; JSONL is a derived,
//! portable projection).

use std::path::{Path, PathBuf};

use tanren_domain::events::{EventEnvelope, RawEventEnvelope};

use crate::Store;
use crate::errors::StoreError;

/// Result statistics returned by [`ingest_phase_events`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayStats {
    pub lines_read: usize,
    pub events_appended: usize,
    pub events_skipped_non_methodology: usize,
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
    #[error("envelope decode error at {path}:{line}: {reason}")]
    EnvelopeDecode {
        path: PathBuf,
        line: usize,
        reason: String,
    },
    #[error("store error: {source}")]
    Store {
        #[from]
        source: StoreError,
    },
}

/// Ingest a JSONL file into the store. Each line is an
/// [`EventEnvelope`]; only methodology-typed payloads are appended,
/// others are counted in `events_skipped_non_methodology` and ignored.
///
/// The operation is not transactional across lines — a mid-file
/// failure leaves already-appended events in the store. Replay is
/// idempotent because the store's `event_id` is a UUID v7 carried
/// in-envelope; replaying the same file twice duplicates events only
/// if callers don't filter by `event_id` first. Lane 0.5 scope does
/// not require dedup; callers who need it should pre-scan.
///
/// # Errors
/// See [`ReplayError`].
pub async fn ingest_phase_events(store: &Store, path: &Path) -> Result<ReplayStats, ReplayError> {
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
        let raw: RawEventEnvelope =
            serde_json::from_str(line).map_err(|source| ReplayError::MalformedLine {
                path: path.to_path_buf(),
                line: line_no,
                reason: source.to_string(),
                raw: line.to_owned(),
            })?;
        let envelope: EventEnvelope =
            raw.try_decode().map_err(|e| ReplayError::EnvelopeDecode {
                path: path.to_path_buf(),
                line: line_no,
                reason: e.to_string(),
            })?;
        if matches!(
            envelope.payload,
            tanren_domain::events::DomainEvent::Methodology { .. }
        ) {
            store.append_methodology_event(&envelope).await?;
            stats.events_appended += 1;
        } else {
            stats.events_skipped_non_methodology += 1;
        }
    }
    Ok(stats)
}
