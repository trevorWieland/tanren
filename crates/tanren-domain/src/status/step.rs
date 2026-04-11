//! Step lifecycle state machine and scheduler-ready state.

use serde::{Deserialize, Serialize};

/// Lifecycle status of a step within a dispatch.
///
/// ```text
/// Pending ─┬─→ Running → Completed | Failed | Cancelled
///          └─→ Cancelled   (queued cancellation)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl StepStatus {
    /// Returns `true` if the step is in a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns `true` if transitioning from `self` to `next` is legal.
    ///
    /// Pending steps may be cancelled directly (dequeued cancellation).
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

impl std::fmt::Display for StepStatus {
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

/// Scheduler readiness for a step in the dispatch graph.
///
/// Distinct from [`StepStatus`]: a step may be `Pending` while still
/// `Blocked` on unmet graph dependencies. Schedulers should only dispatch
/// steps that are `Pending` **and** `Ready`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepReadyState {
    /// Dependencies are not yet satisfied; the scheduler must wait.
    Blocked,
    /// All dependencies satisfied; the scheduler may dispatch.
    Ready,
}

impl std::fmt::Display for StepReadyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocked => f.write_str("blocked"),
            Self::Ready => f.write_str("ready"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states() {
        assert!(!StepStatus::Pending.is_terminal());
        assert!(!StepStatus::Running.is_terminal());
        assert!(StepStatus::Completed.is_terminal());
        assert!(StepStatus::Failed.is_terminal());
        assert!(StepStatus::Cancelled.is_terminal());
    }

    #[test]
    fn valid_transitions() {
        assert!(StepStatus::Pending.can_transition_to(StepStatus::Running));
        assert!(StepStatus::Pending.can_transition_to(StepStatus::Cancelled));
        assert!(StepStatus::Running.can_transition_to(StepStatus::Completed));
        assert!(StepStatus::Running.can_transition_to(StepStatus::Failed));
        assert!(StepStatus::Running.can_transition_to(StepStatus::Cancelled));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!StepStatus::Pending.can_transition_to(StepStatus::Completed));
        assert!(!StepStatus::Pending.can_transition_to(StepStatus::Failed));
        assert!(!StepStatus::Completed.can_transition_to(StepStatus::Running));
    }

    #[test]
    fn terminal_states_have_no_outgoing() {
        for target in [
            StepStatus::Pending,
            StepStatus::Running,
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Cancelled,
        ] {
            assert!(!StepStatus::Completed.can_transition_to(target));
            assert!(!StepStatus::Failed.can_transition_to(target));
            assert!(!StepStatus::Cancelled.can_transition_to(target));
        }
    }

    #[test]
    fn step_status_display_matches_serde() {
        for (status, tag) in [
            (StepStatus::Pending, "pending"),
            (StepStatus::Running, "running"),
            (StepStatus::Completed, "completed"),
            (StepStatus::Failed, "failed"),
            (StepStatus::Cancelled, "cancelled"),
        ] {
            assert_eq!(status.to_string(), tag);
            let json = serde_json::to_string(&status).expect("serialize");
            assert_eq!(json, format!("\"{tag}\""));
        }
    }

    #[test]
    fn step_ready_state_display_matches_serde() {
        for (state, tag) in [
            (StepReadyState::Blocked, "blocked"),
            (StepReadyState::Ready, "ready"),
        ] {
            assert_eq!(state.to_string(), tag);
            let json = serde_json::to_string(&state).expect("serialize");
            assert_eq!(json, format!("\"{tag}\""));
        }
    }
}
