//! Typed authorization, placement, and budget policy decisions for Tanren.
//!
//! Policy returns typed decisions, never transport-layer errors. The runtime
//! and harness crates do not own policy decisions — they consume them as the
//! [`Decision`] enum below.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The outcome of evaluating a policy against an actor and a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// The policy permits the requested action.
    Allow,
    /// The policy denies the requested action. The reason is carried as a
    /// [`DenialReason`] so callers can surface a typed cause without leaking
    /// internal policy state.
    Deny(DenialReason),
}

/// Why a policy returned [`Decision::Deny`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DenialReason {
    /// The actor does not hold a permission required by the resource.
    MissingPermission,
    /// A scoped quota or budget has been exhausted.
    QuotaExhausted,
    /// The runtime placement constraints could not be satisfied.
    PlacementUnsatisfiable,
}

/// Errors raised when policy evaluation itself cannot complete (distinct from
/// a deliberate [`Decision::Deny`]).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PolicyError {
    /// Required policy inputs were missing or malformed.
    #[error("policy evaluation failed: missing input '{0}'")]
    MissingInput(String),
}
