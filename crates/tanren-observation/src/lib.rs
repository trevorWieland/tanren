//! Observation subsystem.
//!
//! Owns dashboards, project overview, work pipeline, quality signals, health
//! signals, forecasts, risk summaries, and reports. Read models served here
//! are derived from the canonical event log via projection workers.
//!
//! ## Provenance model
//!
//! Every observation claim carries provenance metadata identifying source,
//! freshness, completeness, and visibility so consumers can distinguish
//! measured facts from estimated, inferred, unavailable, or redacted values.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// Freshness mark every read model carries. Lets clients distinguish stale
/// from fresh projections without treating realtime delivery as canon.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Freshness {
    /// When this projection was last updated.
    pub as_of: DateTime<Utc>,
}

/// How an observation claim's value was derived.
///
/// Observation should not collapse these into a generic certainty score.
/// If a summary is uncertain the view shows why: missing source, stale
/// projection, redaction, limited history, conflicting signals, or
/// explicit forecast bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimValueKind {
    /// Computed from complete visible source data.
    Measured,
    /// Reported as a range because exact value is not useful or knowable.
    Bounded,
    /// Derived from incomplete but sufficient visible data.
    Estimated,
    /// Derived from source signals with explicit assumptions.
    Inferred,
    /// Source data is absent, hidden, stale, or unsupported.
    Unavailable,
    /// Source data exists but is hidden from the actor.
    Redacted,
}

impl ClaimValueKind {
    /// Stable wire string for this value kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Bounded => "bounded",
            Self::Estimated => "estimated",
            Self::Inferred => "inferred",
            Self::Unavailable => "unavailable",
            Self::Redacted => "redacted",
        }
    }
}

/// Whether a read model's source data is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompletenessState {
    /// All expected source data contributed to this claim.
    Complete,
    /// Some expected source data was missing or unavailable.
    Partial,
    /// No source data was available for this claim.
    Empty,
}

impl CompletenessState {
    /// Stable wire string for this completeness state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Empty => "empty",
        }
    }
}

/// How fresh a read model's projection is relative to its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessState {
    /// Projection was generated from current source data.
    Fresh,
    /// Projection may lag behind the latest source events.
    Stale,
    /// Freshness cannot be determined (no checkpoint available).
    Unknown,
}

impl FreshnessState {
    /// Stable wire string for this freshness state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
            Self::Unknown => "unknown",
        }
    }
}

/// Whether a claim's data is visible to the requesting actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityState {
    /// Data is fully visible to the actor.
    Visible,
    /// Data exists but is hidden from the actor's scope.
    Hidden,
    /// Data was redacted before reaching the actor.
    Redacted,
}

impl VisibilityState {
    /// Stable wire string for this visibility state.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Redacted => "redacted",
        }
    }
}

/// Errors raised by observation queries.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ObservationError {
    /// The requested read model has no projection yet.
    #[error("read model not yet projected: {0}")]
    NotProjected(String),
}
