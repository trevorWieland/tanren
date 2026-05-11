//! Read-model freshness metadata shared across organization responses.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_observation::{ClaimValueKind, CompletenessState, FreshnessState, VisibilityState};
use utoipa::ToSchema;

/// Freshness and provenance metadata for organization list read models.
///
/// Aligned with the Observation Claim Model (see
/// docs/architecture/subsystems/observation.md).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadModelFreshness {
    /// Logical projection/read-model name serving this response.
    pub projection: String,
    /// Projection checkpoint identifier when available.
    pub checkpoint: Option<String>,
    /// Response-generation timestamp from the read path.
    pub generated_at: DateTime<Utc>,
    /// Cursor associated with this read model page when available.
    pub cursor: Option<String>,
    /// Source subsystem that produced the underlying data.
    pub source: String,
    /// How the claim value was derived.
    pub value_kind: ClaimValueKind,
    /// Whether source data is complete, partial, or empty.
    pub completeness: CompletenessState,
    /// Whether the claim is fresh, stale, or unknown.
    pub freshness_state: FreshnessState,
    /// Visibility of this claim for the requesting actor.
    pub visibility: VisibilityState,
}
