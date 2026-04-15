//! Contract-level error types and error mapping.
//!
//! [`ContractError`] is the typed library error used within the contract
//! and app-services crates. [`ErrorResponse`] is the serializable wire
//! error returned to transport interfaces.

use serde::{Deserialize, Serialize};
use tanren_domain::{DomainError, PolicyReasonCode};

/// Machine-readable wire error code shared by all transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    NotFound,
    SchemaNotReady,
    InvalidTransition,
    ContentionConflict,
    Conflict,
    PolicyDenied,
    Internal,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::SchemaNotReady => "schema_not_ready",
            Self::InvalidTransition => "invalid_transition",
            Self::ContentionConflict => "contention_conflict",
            Self::Conflict => "conflict",
            Self::PolicyDenied => "policy_denied",
            Self::Internal => "internal",
        };
        f.write_str(text)
    }
}

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

    /// The backing database schema is not initialized for this operation.
    #[error("schema not ready: {reason}")]
    SchemaNotReady {
        /// Why the schema is considered not ready.
        reason: String,
    },

    /// The requested lifecycle transition is invalid.
    #[error("invalid transition on {entity}: {from} -> {to}")]
    InvalidTransition {
        /// The entity in the invalid transition.
        entity: String,
        /// The transition's source state.
        from: String,
        /// The transition's target state.
        to: String,
    },

    /// A typed contention conflict.
    #[error("contention conflict in {operation}: {reason}")]
    ContentionConflict {
        /// Operation where contention was observed.
        operation: String,
        /// Description of the contention.
        reason: String,
    },

    /// A domain-level conflict (e.g. invalid state transition).
    #[error("conflict: {reason}")]
    Conflict {
        /// Description of the conflict.
        reason: String,
    },

    /// Policy denied the operation.
    #[error("policy denied")]
    PolicyDenied {
        /// Machine-safe policy reason code when available.
        reason_code: Option<PolicyReasonCode>,
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
    pub code: ErrorCode,
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
        match err {
            ContractError::InvalidField { field, reason } => Self {
                code: ErrorCode::InvalidInput,
                message: format!("invalid field `{field}`: {reason}"),
                details: None,
            },
            ContractError::NotFound { entity, id } => Self {
                code: ErrorCode::NotFound,
                message: format!("{entity} not found: {id}"),
                details: None,
            },
            ContractError::SchemaNotReady { reason } => Self {
                code: ErrorCode::SchemaNotReady,
                message: format!("schema not ready: {reason}"),
                details: None,
            },
            ContractError::InvalidTransition { entity, from, to } => Self {
                code: ErrorCode::InvalidTransition,
                message: format!("invalid transition on {entity}: {from} -> {to}"),
                details: None,
            },
            ContractError::ContentionConflict { operation, reason } => Self {
                code: ErrorCode::ContentionConflict,
                message: format!("contention conflict in {operation}: {reason}"),
                details: None,
            },
            ContractError::Conflict { reason } => Self {
                code: ErrorCode::Conflict,
                message: format!("conflict: {reason}"),
                details: None,
            },
            ContractError::PolicyDenied { reason_code } => Self {
                code: ErrorCode::PolicyDenied,
                message: "policy denied".to_owned(),
                details: reason_code.map(|code| {
                    serde_json::json!({
                        "reason_code": code.to_string(),
                    })
                }),
            },
            ContractError::Internal { message } => Self {
                code: ErrorCode::Internal,
                message: format!("internal error: {message}"),
                details: None,
            },
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
            DomainError::PolicyDenied { .. }
            | DomainError::BudgetExceeded { .. }
            | DomainError::QuotaExhausted { .. } => Self::PolicyDenied { reason_code: None },
            DomainError::InvalidTransition { entity, from, to } => Self::InvalidTransition {
                entity: entity.to_string(),
                from: from.clone(),
                to: to.clone(),
            },
            DomainError::Conflict { .. }
            | DomainError::ConcurrentExecute { .. }
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
