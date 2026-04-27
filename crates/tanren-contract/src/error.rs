//! Contract-level error types and error mapping.
//!
//! [`ContractError`] is the typed library error used within the contract
//! and app-services crates. [`ErrorResponse`] is the serializable wire
//! error returned to transport interfaces.

use serde::{Deserialize, Serialize};
use tanren_domain::{DomainError, EntityRef, PolicyReasonCode};
use uuid::Uuid;

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

/// Machine-safe classification of CLI argument-parsing failures.
///
/// Produced by transport binaries when a clap (or equivalent) parser
/// rejects user input. The wire payload is stable across clap
/// versions because the transport maps `clap::error::ErrorKind` →
/// this enum rather than shipping raw parser prose. Raw user input
/// (which may contain secrets typed into the wrong slot) is never
/// echoed into the wire body via this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliParseReasonCode {
    /// A required argument was not supplied.
    MissingRequiredArgument,
    /// A value did not match the argument's declared type or allowlist.
    InvalidValue,
    /// The subcommand path was unrecognized.
    InvalidSubcommand,
    /// An unknown argument/flag was supplied.
    UnknownArgument,
    /// Two arguments conflicted.
    ArgumentConflict,
    /// Too many values supplied for a single argument.
    TooManyValues,
    /// Not enough values supplied for an argument that requires multiple.
    TooFewValues,
    /// A custom value-validator rejected the input.
    ValueValidation,
    /// `--flag=` syntax expected but missing `=`.
    NoEquals,
    /// Any other clap parser failure that does not map to a more specific code.
    Format,
}

impl std::fmt::Display for CliParseReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::MissingRequiredArgument => "missing_required_argument",
            Self::InvalidValue => "invalid_value",
            Self::InvalidSubcommand => "invalid_subcommand",
            Self::UnknownArgument => "unknown_argument",
            Self::ArgumentConflict => "argument_conflict",
            Self::TooManyValues => "too_many_values",
            Self::TooFewValues => "too_few_values",
            Self::ValueValidation => "value_validation",
            Self::NoEquals => "no_equals",
            Self::Format => "format",
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

    /// A transport-level argument parser rejected the input.
    ///
    /// Unlike [`Self::InvalidField`], this variant does not carry any
    /// user-typed text in the wire message. `field` must already be
    /// allowlisted by the caller against the set of known long flag
    /// names — untrusted strings stay at the transport boundary.
    #[error("invalid cli args")]
    InvalidArgs {
        /// Allowlisted long-flag name the parser rejected, if known.
        field: Option<String>,
        /// Machine-safe classification of the parser failure.
        reason_code: CliParseReasonCode,
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
    /// Optional typed details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
}

/// Typed detail payload for wire errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorDetails {
    PolicyDenied {
        reason_code: PolicyReasonCode,
    },
    Internal {
        correlation_id: Uuid,
    },
    /// CLI/API argument-parser rejection; stable wire shape.
    InvalidArgs {
        /// Allowlisted long-flag name the parser rejected, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<String>,
        /// Machine-safe classification of the parser failure.
        reason_code: CliParseReasonCode,
    },
}

/// Build a canonical internal error response with optional correlation details.
///
/// The callback should persist correlation metadata and return `Ok(())` only
/// when the correlation ID is durably recorded.
pub fn internal_error_response_with_correlation<E>(
    emit: impl FnOnce(Uuid) -> Result<(), E>,
) -> ErrorResponse {
    let correlation_id = Uuid::now_v7();
    let details = if emit(correlation_id).is_ok() {
        Some(ErrorDetails::Internal { correlation_id })
    } else {
        None
    };

    ErrorResponse {
        code: ErrorCode::Internal,
        message: "internal error".to_owned(),
        details,
    }
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
            ContractError::InvalidArgs { field, reason_code } => Self {
                code: ErrorCode::InvalidInput,
                message: "invalid cli args".to_owned(),
                details: Some(ErrorDetails::InvalidArgs { field, reason_code }),
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
                details: reason_code.map(|reason_code| ErrorDetails::PolicyDenied { reason_code }),
            },
            ContractError::Internal { .. } => Self {
                code: ErrorCode::Internal,
                message: "internal error".to_owned(),
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
                id: entity_ref_id(entity),
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

fn entity_ref_id(entity: &EntityRef) -> String {
    match entity {
        EntityRef::Dispatch(id) => id.to_string(),
        EntityRef::Step(id) => id.to_string(),
        EntityRef::Lease(id) => id.to_string(),
        EntityRef::User(id) => id.to_string(),
        EntityRef::Org(id) => id.to_string(),
        EntityRef::Team(id) => id.to_string(),
        EntityRef::Project(id) => id.to_string(),
        EntityRef::ApiKey(id) => id.to_string(),
        EntityRef::Spec(id) => id.to_string(),
        EntityRef::Task(id) => id.to_string(),
        EntityRef::Finding(id) => id.to_string(),
        EntityRef::Signpost(id) => id.to_string(),
        EntityRef::Issue(id) => id.to_string(),
    }
}
