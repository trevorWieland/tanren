//! Observation subsystem.
//!
//! Owns dashboards, project overview, work pipeline, quality signals, health
//! signals, forecasts, risk summaries, and reports. Read models served here
//! are derived from the canonical event log via projection workers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Freshness mark every read model carries. Lets clients distinguish stale
/// from fresh projections without treating realtime delivery as canon.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Freshness {
    /// When this projection was last updated.
    pub as_of: DateTime<Utc>,
}

/// Errors raised by observation queries.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ObservationError {
    /// The requested read model has no projection yet.
    #[error("read model not yet projected: {0}")]
    NotProjected(String),
}
