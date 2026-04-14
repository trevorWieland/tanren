//! Contract-level error types and error mapping.
//!
//! [`ContractError`] is the typed library error used within the contract
//! and app-services crates. [`ErrorResponse`] is the serializable wire
//! error returned to transport interfaces.

use serde::{Deserialize, Serialize};
use tanren_domain::DomainError;

/// Typed contract-level error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ContractError {
    /// A request field failed validation.
    #[error("invalid field `{field}`: {reason}")]
    InvalidField {
        /// The field that failed validation.
        field: String,
        /// Why the value was rejected.
        reason: String,
    },

    /// The requested entity was not found.
    #[error("{entity} not found: {id}")]
    NotFound {
        /// Kind of entity (e.g. "dispatch", "step").
        entity: String,
        /// Identifier of the missing entity.
        id: String,
    },

    /// A domain-level conflict (e.g. invalid state transition).
    #[error("conflict: {reason}")]
    Conflict {
        /// Description of the conflict.
        reason: String,
    },

    /// Policy denied the operation.
    #[error("policy denied: {reason}")]
    PolicyDenied {
        /// Reason the policy denied the action.
        reason: String,
    },

    /// An internal infrastructure error.
    #[error("internal error: {message}")]
    Internal {
        /// Description of the internal failure.
        message: String,
    },
}

/// Serializable wire error returned to all transport interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ErrorResponse {}

impl From<ContractError> for ErrorResponse {
    fn from(err: ContractError) -> Self {
        let (code, message) = match &err {
            ContractError::InvalidField { .. } => ("invalid_input", err.to_string()),
            ContractError::NotFound { .. } => ("not_found", err.to_string()),
            ContractError::Conflict { .. } => ("conflict", err.to_string()),
            ContractError::PolicyDenied { .. } => ("policy_denied", err.to_string()),
            ContractError::Internal { .. } => ("internal", err.to_string()),
        };
        Self {
            code: code.to_owned(),
            message,
            details: None,
        }
    }
}

impl From<DomainError> for ContractError {
    fn from(err: DomainError) -> Self {
        match &err {
            DomainError::NotFound { entity } => Self::NotFound {
                entity: entity.kind().to_string(),
                id: entity.to_string(),
            },
            DomainError::InvalidValue { field, reason } => Self::InvalidField {
                field: field.clone(),
                reason: reason.clone(),
            },
            DomainError::PolicyDenied { reason, .. } => Self::PolicyDenied {
                reason: reason.clone(),
            },
            DomainError::BudgetExceeded { limit, current } => Self::PolicyDenied {
                reason: format!("budget exceeded: limit={limit}, current={current}"),
            },
            DomainError::QuotaExhausted { resource, limit } => Self::PolicyDenied {
                reason: format!("quota exhausted for {resource}: limit={limit}"),
            },
            DomainError::InvalidTransition { .. } | DomainError::Conflict { .. } => {
                Self::Conflict {
                    reason: err.to_string(),
                }
            }
            DomainError::ConcurrentExecute { .. }
            | DomainError::PostTeardownExecute { .. }
            | DomainError::ActiveExecuteTeardown { .. }
            | DomainError::DuplicateTeardown { .. } => Self::Conflict {
                reason: err.to_string(),
            },
            DomainError::PreconditionFailed { reason } => Self::Conflict {
                reason: reason.clone(),
            },
        }
    }
}
