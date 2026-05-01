//! Quality Controls subsystem.
//!
//! Owns automated gates, audit rubric checks, standards adherence checks,
//! run-demo critique semantics, and task/spec guards. Each gate returns a
//! typed [`GateResult`] so orchestration can route findings without
//! interpreting transport errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Outcome of evaluating a single gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GateResult {
    /// The gate passed.
    Pass,
    /// The gate failed; the string is a human-readable summary suitable for
    /// surfacing in review feedback.
    Fail(String),
    /// The gate was not applicable to this artifact.
    NotApplicable,
}

/// Trait every quality gate implements.
#[async_trait::async_trait]
pub trait Gate: Send + Sync {
    /// Stable name of this gate (used in audit records).
    fn name(&self) -> &str;
}

/// Errors raised by gate evaluation infrastructure (distinct from a
/// deliberate [`GateResult::Fail`]).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum QualityControlsError {
    /// The gate could not be invoked.
    #[error("gate invocation failed: {0}")]
    InvocationFailed(String),
}
