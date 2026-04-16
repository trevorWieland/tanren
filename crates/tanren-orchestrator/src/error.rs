//! Orchestrator-level error types.

use tanren_domain::{DomainError, PolicyDecisionRecord};
use tanren_store::StoreError;

/// Errors produced by orchestrator operations.
///
/// Wraps domain and store errors, adding an orchestrator-specific
/// `PolicyDenied` variant for when the policy engine refuses a request.
#[derive(Debug, thiserror::Error)]
pub enum OrchestratorError {
    /// A domain-level error (validation, guards, preconditions).
    #[error(transparent)]
    Domain(#[from] DomainError),

    /// A store-level error (database, conversion, conflict).
    #[error(transparent)]
    Store(#[from] StoreError),

    /// The policy engine denied the requested operation.
    #[error("policy denied: {}", .decision.reason.as_deref().unwrap_or("no reason provided"))]
    PolicyDenied {
        /// The full policy decision record for audit.
        decision: Box<PolicyDecisionRecord>,
    },
}
