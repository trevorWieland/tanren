//! Lease lifecycle state machine.

use serde::{Deserialize, Serialize};

/// Lifecycle status of an execution lease.
///
/// ```text
/// Happy path:
///     Requested → Provisioning → Ready → Running → Idle → Draining → Released
///
/// Cancel path:
///     Running | Ready → Draining → Released
///
/// Failure path:
///     any non-terminal → Failed → Draining → Released
/// ```
///
/// Only [`Self::Released`] is terminal — `Failed` is intermediate because
/// post-failure cleanup must still run. A "failed but fully cleaned up"
/// lease is visible as `Released` with a preceding `LeaseFailed` event in
/// the event history, distinct from "failed and potentially leaking"
/// which would leave the lease stuck at `Failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseStatus {
    Requested,
    Provisioning,
    Ready,
    Running,
    Idle,
    Draining,
    Released,
    Failed,
}

impl LeaseStatus {
    /// Returns `true` if the lease is in a terminal state.
    ///
    /// Only [`Self::Released`] is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Released)
    }

    /// Returns `true` if transitioning from `self` to `next` is legal.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        // Any non-terminal state except Failed itself can transition to
        // Failed. Failed is a recovery branch, not a loop.
        if next == Self::Failed {
            return !matches!(self, Self::Released | Self::Failed);
        }

        matches!(
            (self, next),
            // Happy path
            (Self::Requested, Self::Provisioning)
                | (Self::Provisioning, Self::Ready)
                | (Self::Ready | Self::Idle, Self::Running | Self::Draining)
                | (Self::Running, Self::Idle | Self::Draining)
                | (Self::Draining, Self::Released)
                // Post-failure cleanup: Failed must still drain and release.
                | (Self::Failed, Self::Draining)
        )
    }
}

impl std::fmt::Display for LeaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requested => f.write_str("requested"),
            Self::Provisioning => f.write_str("provisioning"),
            Self::Ready => f.write_str("ready"),
            Self::Running => f.write_str("running"),
            Self::Idle => f.write_str("idle"),
            Self::Draining => f.write_str("draining"),
            Self::Released => f.write_str("released"),
            Self::Failed => f.write_str("failed"),
        }
    }
}
