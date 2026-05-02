//! Orchestration subsystem.
//!
//! Owns spec lifecycle, task lifecycle, phase execution, control-batch
//! scheduling, review feedback routing, merge routing, and cleanup. Decides
//! the *what next* of any active spec; runtime decides the *where it runs*.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// High-level state of a spec or task as it traverses the orchestration
/// pipeline. Concrete transitions are owned by the slices that implement
/// each phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LifecycleStatus {
    /// Pending pickup by a control batch.
    Pending,
    /// Currently in flight.
    Active,
    /// Completed successfully.
    Done,
    /// Halted for a reason captured separately.
    Blocked,
}

/// Errors raised by orchestration operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OrchestrationError {
    /// A requested transition is not legal from the current state.
    #[error("illegal transition: {from:?} -> {to:?}")]
    IllegalTransition {
        /// Source state.
        from: LifecycleStatus,
        /// Attempted target state.
        to: LifecycleStatus,
    },
}
