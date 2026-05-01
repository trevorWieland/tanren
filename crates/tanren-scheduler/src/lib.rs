//! Cron-style schedule evaluation for Tanren.
//!
//! The [`Scheduler`] trait is the seam: implementations decide *when* a
//! schedule is next due. Dispatch — actually running the work — belongs to
//! the runtime and orchestration layers. This crate intentionally has no
//! `tokio` dependency; trait impls are async via `async_trait` so callers
//! choose their own runtime.

use chrono::{DateTime, Utc};
use thiserror::Error;

/// Computes the next due time for a schedule expression.
#[async_trait::async_trait]
pub trait Scheduler: Send + Sync {
    /// Return the next time `expression` is due to fire after `after`.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::InvalidExpression`] if the expression cannot
    /// be parsed.
    async fn next_due_after(
        &self,
        expression: &str,
        after: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, SchedulerError>;
}

/// Errors raised by scheduler implementations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SchedulerError {
    /// The schedule expression failed to parse.
    #[error("invalid schedule expression: {0}")]
    InvalidExpression(String),
}
