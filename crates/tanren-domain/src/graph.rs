//! Graph-native scheduling primitives.
//!
//! The orchestrator models every dispatch as a revisioned task graph.
//! [`GraphRevision`] is a typed version counter that advances each time
//! the planner emits a new graph (initial planning, replanning on
//! failure, scope change). Commands and events carry the revision they
//! belong to so stale enqueues — an `EnqueueStep` that arrives after a
//! replan — can be recognized and rejected.

use serde::{Deserialize, Serialize};

/// Typed version counter for a dispatch's task graph.
///
/// Advances monotonically. Comparisons model staleness: a command
/// arriving with a revision less than the dispatch's current revision
/// is stale and must be rejected by the orchestrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphRevision(u32);

impl GraphRevision {
    /// Revision zero — used only by tests and initial dispatch creation
    /// before any planning has occurred.
    pub const ZERO: Self = Self(0);

    /// The first real planning revision.
    pub const INITIAL: Self = Self(1);

    /// Wrap a raw `u32` as a [`GraphRevision`].
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Return the inner `u32`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Return the next revision, saturating at [`u32::MAX`].
    ///
    /// A saturating bump is safe because the u32 space is effectively
    /// unbounded for a single dispatch — hitting `u32::MAX` means the
    /// planner has emitted four billion graphs for one dispatch, which
    /// is a planner bug, not a counter wraparound.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Returns `true` if `self` is stale relative to `current` — i.e.
    /// the command's graph revision is older than the dispatch's
    /// current revision.
    #[must_use]
    pub const fn is_stale_relative_to(self, current: Self) -> bool {
        self.0 < current.0
    }
}

impl std::fmt::Display for GraphRevision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rev{}", self.0)
    }
}

impl From<u32> for GraphRevision {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
