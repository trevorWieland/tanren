//! Planning subsystem.
//!
//! Owns product, behavior catalog, architecture records, roadmap graph,
//! decisions, planning proposals, and assumption tracking. Concrete planning
//! handlers arrive with the R-* slices that implement product / planning /
//! roadmap behaviors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable identifier for any planning artifact (behavior, roadmap node,
/// decision, proposal). Format is enforced by the artifact's owning skill.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanningId(String);

impl PlanningId {
    /// Wrap a raw string as a planning id without validation. Validation
    /// arrives with the slice that introduces the artifact type.
    #[must_use]
    pub const fn from_string(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Errors raised by planning operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PlanningError {
    /// A planning artifact id failed format validation.
    #[error("invalid planning id: {0}")]
    InvalidId(String),
}
