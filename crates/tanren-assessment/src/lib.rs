//! Assessment subsystem.
//!
//! Owns spec-independent implementation analysis: findings about the live
//! implementation, recommendations, intake classification, and routing into
//! the planning queue.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Severity of an assessment finding. Routing rules consume this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Severity {
    /// Informational — surfaced but does not gate anything.
    Info,
    /// Worth fixing but not blocking.
    Warning,
    /// Blocks acceptance until resolved.
    Blocking,
}

/// A single finding produced by an assessment pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable id for cross-run deduplication.
    pub id: String,
    /// Human-readable summary.
    pub summary: String,
    /// Severity classification.
    pub severity: Severity,
}

/// Errors raised by assessment operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AssessmentError {
    /// The assessment pass could not produce a result.
    #[error("assessment failed: {0}")]
    Failed(String),
}
