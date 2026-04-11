//! Pure guard functions that validate state transitions from step lists.
//!
//! These operate on `&[StepView]` without touching a store, keeping guard
//! logic in the domain layer where it is testable in isolation.

use crate::errors::DomainError;
use crate::status::{StepStatus, StepType};
use crate::views::StepView;

/// Validate that a new execute step may be enqueued.
///
/// Blocks if:
/// - Any EXECUTE step is currently Pending or Running (concurrent execute).
/// - Any TEARDOWN step exists in Pending, Running, or Completed state
///   (post-teardown execute).
///
/// # Errors
///
/// Returns [`DomainError::ConcurrentExecute`] or
/// [`DomainError::PostTeardownExecute`].
pub fn check_execute_guards(steps: &[StepView]) -> Result<(), DomainError> {
    for step in steps {
        if step.step_type == StepType::Execute
            && matches!(step.status, StepStatus::Pending | StepStatus::Running)
        {
            return Err(DomainError::ConcurrentExecute {
                dispatch_id: step.dispatch_id,
            });
        }

        if step.step_type == StepType::Teardown
            && matches!(
                step.status,
                StepStatus::Pending | StepStatus::Running | StepStatus::Completed
            )
        {
            return Err(DomainError::PostTeardownExecute {
                dispatch_id: step.dispatch_id,
            });
        }
    }

    Ok(())
}

/// Validate that a teardown step may be enqueued.
///
/// Blocks if:
/// - Any EXECUTE step is currently Pending or Running (active execute).
/// - Any TEARDOWN step already exists in a non-retriable state.
///
/// When `allow_retry_after_failure` is `true`, a previously failed teardown
/// is permitted (only Failed teardowns are ignored). When `false`, any
/// existing teardown in Pending, Running, or Completed state blocks.
///
/// # Errors
///
/// Returns [`DomainError::ActiveExecuteTeardown`] or
/// [`DomainError::DuplicateTeardown`].
pub fn check_teardown_guards(
    steps: &[StepView],
    allow_retry_after_failure: bool,
) -> Result<(), DomainError> {
    for step in steps {
        if step.step_type == StepType::Execute
            && matches!(step.status, StepStatus::Pending | StepStatus::Running)
        {
            return Err(DomainError::ActiveExecuteTeardown {
                dispatch_id: step.dispatch_id,
            });
        }

        if step.step_type == StepType::Teardown {
            let blocked = if allow_retry_after_failure {
                // Only Failed teardowns permit a retry; anything else (pending,
                // running, completed, cancelled) blocks a new attempt so we
                // never issue duplicate cleanups.
                step.status != StepStatus::Failed
            } else {
                matches!(
                    step.status,
                    StepStatus::Pending | StepStatus::Running | StepStatus::Completed
                )
            };

            if blocked {
                return Err(DomainError::DuplicateTeardown {
                    dispatch_id: step.dispatch_id,
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::ids::{DispatchId, StepId};
    use crate::status::StepReadyState;

    fn make_step(step_type: StepType, status: StepStatus) -> StepView {
        let dispatch_id = DispatchId::new();
        StepView {
            step_id: StepId::new(),
            dispatch_id,
            step_type,
            step_sequence: 1,
            lane: None,
            status,
            ready_state: StepReadyState::Ready,
            depends_on: Vec::new(),
            graph_revision: 1,
            worker_id: None,
            payload: None,
            result: None,
            error: None,
            retry_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // -- Execute guards ---------------------------------------------------

    #[test]
    fn execute_guard_passes_on_empty_steps() {
        assert!(check_execute_guards(&[]).is_ok());
    }

    #[test]
    fn execute_guard_passes_with_completed_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Completed)];
        assert!(check_execute_guards(&steps).is_ok());
    }

    #[test]
    fn execute_guard_passes_with_failed_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Failed)];
        assert!(check_execute_guards(&steps).is_ok());
    }

    #[test]
    fn execute_guard_blocks_pending_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Pending)];
        let err = check_execute_guards(&steps).expect_err("guard should reject");
        assert!(matches!(err, DomainError::ConcurrentExecute { .. }));
    }

    #[test]
    fn execute_guard_blocks_running_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Running)];
        let err = check_execute_guards(&steps).expect_err("guard should reject");
        assert!(matches!(err, DomainError::ConcurrentExecute { .. }));
    }

    #[test]
    fn execute_guard_blocks_after_pending_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Pending)];
        let err = check_execute_guards(&steps).expect_err("guard should reject");
        assert!(matches!(err, DomainError::PostTeardownExecute { .. }));
    }

    #[test]
    fn execute_guard_blocks_after_completed_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Completed)];
        let err = check_execute_guards(&steps).expect_err("guard should reject");
        assert!(matches!(err, DomainError::PostTeardownExecute { .. }));
    }

    #[test]
    fn execute_guard_allows_after_failed_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Failed)];
        assert!(check_execute_guards(&steps).is_ok());
    }

    #[test]
    fn execute_guard_ignores_provision_steps() {
        let steps = [make_step(StepType::Provision, StepStatus::Running)];
        assert!(check_execute_guards(&steps).is_ok());
    }

    // -- Teardown guards --------------------------------------------------

    #[test]
    fn teardown_guard_passes_on_empty_steps() {
        assert!(check_teardown_guards(&[], false).is_ok());
    }

    #[test]
    fn teardown_guard_blocks_pending_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Pending)];
        let err = check_teardown_guards(&steps, false).expect_err("guard should reject");
        assert!(matches!(err, DomainError::ActiveExecuteTeardown { .. }));
    }

    #[test]
    fn teardown_guard_blocks_running_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Running)];
        let err = check_teardown_guards(&steps, false).expect_err("guard should reject");
        assert!(matches!(err, DomainError::ActiveExecuteTeardown { .. }));
    }

    #[test]
    fn teardown_guard_allows_completed_execute() {
        let steps = [make_step(StepType::Execute, StepStatus::Completed)];
        assert!(check_teardown_guards(&steps, false).is_ok());
    }

    #[test]
    fn teardown_guard_blocks_duplicate_pending_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Pending)];
        let err = check_teardown_guards(&steps, false).expect_err("guard should reject");
        assert!(matches!(err, DomainError::DuplicateTeardown { .. }));
    }

    #[test]
    fn teardown_guard_blocks_duplicate_completed_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Completed)];
        let err = check_teardown_guards(&steps, false).expect_err("guard should reject");
        assert!(matches!(err, DomainError::DuplicateTeardown { .. }));
    }

    #[test]
    fn teardown_guard_allows_failed_teardown_without_retry() {
        // Without retry flag, a failed teardown does NOT block.
        let steps = [make_step(StepType::Teardown, StepStatus::Failed)];
        assert!(check_teardown_guards(&steps, false).is_ok());
    }

    #[test]
    fn teardown_guard_allows_retry_after_failed_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Failed)];
        assert!(check_teardown_guards(&steps, true).is_ok());
    }

    #[test]
    fn teardown_guard_retry_blocks_completed_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Completed)];
        let err = check_teardown_guards(&steps, true).expect_err("guard should reject");
        assert!(matches!(err, DomainError::DuplicateTeardown { .. }));
    }

    #[test]
    fn teardown_guard_retry_blocks_pending_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Pending)];
        let err = check_teardown_guards(&steps, true).expect_err("guard should reject");
        assert!(matches!(err, DomainError::DuplicateTeardown { .. }));
    }

    #[test]
    fn teardown_guard_retry_blocks_cancelled_teardown() {
        // Regression: only Failed teardowns permit a retry attempt.
        let steps = [make_step(StepType::Teardown, StepStatus::Cancelled)];
        let err =
            check_teardown_guards(&steps, true).expect_err("cancelled teardown should block retry");
        assert!(matches!(err, DomainError::DuplicateTeardown { .. }));
    }

    #[test]
    fn teardown_guard_retry_blocks_running_teardown() {
        let steps = [make_step(StepType::Teardown, StepStatus::Running)];
        let err =
            check_teardown_guards(&steps, true).expect_err("running teardown should block retry");
        assert!(matches!(err, DomainError::DuplicateTeardown { .. }));
    }
}
