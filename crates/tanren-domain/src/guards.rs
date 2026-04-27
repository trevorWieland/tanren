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
