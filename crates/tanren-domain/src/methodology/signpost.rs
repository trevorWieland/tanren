//! Signposts — narrative waypoints recorded during task implementation.
//!
//! A [`Signpost`] captures a tricky problem, what the implementer tried,
//! and how (or whether) it was resolved. Per
//! `docs/architecture/evidence-schemas.md`, signposts live in
//! `signposts.md` with typed frontmatter plus a free-form markdown body;
//! the `entries` list is managed exclusively via `add_signpost` /
//! `update_signpost_status` tool calls.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{SignpostId, SpecId, TaskId};
use crate::validated::NonEmptyString;

/// Lifecycle status of a signpost entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignpostStatus {
    /// Still blocking or unclear.
    Unresolved,
    /// Fixed; resolution captured.
    Resolved,
    /// Acknowledged and accepted without resolution.
    Deferred,
    /// Known architectural limitation — no fix planned.
    ArchitecturalConstraint,
}

impl SignpostStatus {
    /// True if the status indicates an outstanding issue.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Unresolved)
    }
}

/// Canonical signpost record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Signpost {
    pub id: SignpostId,
    pub spec_id: SpecId,
    pub task_id: Option<TaskId>,
    pub status: SignpostStatus,
    pub problem: NonEmptyString,
    pub evidence: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tried: Vec<String>,
    pub solution: Option<String>,
    pub resolution: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_affected: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
