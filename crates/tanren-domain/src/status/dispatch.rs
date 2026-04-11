//! Dispatch lifecycle state machine.

use serde::{Deserialize, Serialize};

/// Lifecycle status of a dispatch.
///
/// ```text
/// Pending ─┬─→ Running → Completed | Failed | Cancelled
///          └─→ Cancelled   (queued cancellation)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl DispatchStatus {
    /// Returns `true` if the dispatch is in a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns `true` if transitioning from `self` to `next` is legal.
    ///
    /// Queued dispatches may be cancelled without starting. Running
    /// dispatches may complete, fail, or be cancelled.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::Completed | Self::Failed | Self::Cancelled
                )
        )
    }
}

impl std::fmt::Display for DispatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => f.write_str("pending"),
            Self::Running => f.write_str("running"),
            Self::Completed => f.write_str("completed"),
            Self::Failed => f.write_str("failed"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states() {
        assert!(!DispatchStatus::Pending.is_terminal());
        assert!(!DispatchStatus::Running.is_terminal());
        assert!(DispatchStatus::Completed.is_terminal());
        assert!(DispatchStatus::Failed.is_terminal());
        assert!(DispatchStatus::Cancelled.is_terminal());
    }

    #[test]
    fn valid_transitions() {
        assert!(DispatchStatus::Pending.can_transition_to(DispatchStatus::Running));
        assert!(DispatchStatus::Pending.can_transition_to(DispatchStatus::Cancelled));
        assert!(DispatchStatus::Running.can_transition_to(DispatchStatus::Completed));
        assert!(DispatchStatus::Running.can_transition_to(DispatchStatus::Failed));
        assert!(DispatchStatus::Running.can_transition_to(DispatchStatus::Cancelled));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!DispatchStatus::Pending.can_transition_to(DispatchStatus::Completed));
        assert!(!DispatchStatus::Pending.can_transition_to(DispatchStatus::Failed));
        assert!(!DispatchStatus::Completed.can_transition_to(DispatchStatus::Running));
        assert!(!DispatchStatus::Failed.can_transition_to(DispatchStatus::Running));
        assert!(!DispatchStatus::Cancelled.can_transition_to(DispatchStatus::Pending));
    }

    #[test]
    fn no_self_transitions() {
        assert!(!DispatchStatus::Pending.can_transition_to(DispatchStatus::Pending));
        assert!(!DispatchStatus::Running.can_transition_to(DispatchStatus::Running));
    }

    #[test]
    fn terminal_states_have_no_outgoing() {
        for target in [
            DispatchStatus::Pending,
            DispatchStatus::Running,
            DispatchStatus::Completed,
            DispatchStatus::Failed,
            DispatchStatus::Cancelled,
        ] {
            assert!(!DispatchStatus::Completed.can_transition_to(target));
            assert!(!DispatchStatus::Failed.can_transition_to(target));
            assert!(!DispatchStatus::Cancelled.can_transition_to(target));
        }
    }
}
