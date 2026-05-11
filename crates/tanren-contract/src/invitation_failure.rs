//! Invitation failure taxonomy.
//!
//! Closed error-code and failure-reason enums for invitation operations.
//! Every interface (api, mcp, cli, tui, web) projects these types into
//! the same `{code, summary}` wire shape so callers match on `code`
//! regardless of transport. Maps onto the shared error body contract
//! from `docs/architecture/subsystems/interfaces.md` "Error Taxonomy".

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Closed taxonomy of invitation-flow failures.
///
/// Maps onto the shared `{code, summary}` error body from the interfaces
/// architecture. Every interface projects an `InvitationFailureReason`
/// into the same wire shape so callers match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InvitationFailureReason {
    AuthRequired,
    PermissionDenied,
    ValidationFailed,
    InvitationNotFound,
    InvitationAlreadyConsumed,
    PersonalContextNotAllowed,
}

impl InvitationFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::ValidationFailed => "validation_failed",
            Self::InvitationNotFound => "invitation_not_found",
            Self::InvitationAlreadyConsumed => "invitation_already_consumed",
            Self::PersonalContextNotAllowed => "personal_context_not_allowed",
        }
    }

    /// Human-readable wire `summary`.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AuthRequired => {
                "The request requires authentication and the supplied session is missing or expired."
            }
            Self::PermissionDenied => {
                "The authenticated actor lacks permission to perform this action."
            }
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::InvitationNotFound => {
                "The invitation does not exist or is not visible to the caller."
            }
            Self::InvitationAlreadyConsumed => {
                "The invitation has already been accepted or revoked."
            }
            Self::PersonalContextNotAllowed => {
                "Invitation operations require an organizational context."
            }
        }
    }

    /// Recommended HTTP status for api / mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AuthRequired => 401,
            Self::PermissionDenied => 403,
            Self::ValidationFailed | Self::PersonalContextNotAllowed => 400,
            Self::InvitationNotFound => 404,
            Self::InvitationAlreadyConsumed => 410,
        }
    }
}

/// Error-code taxonomy for invitation operations across all interfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InvitationFailureCode {
    AuthRequired,
    PermissionDenied,
    ValidationFailed,
    InvitationNotFound,
    InvitationAlreadyConsumed,
    PersonalContextNotAllowed,
    InternalError,
}

impl InvitationFailureCode {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::ValidationFailed => "validation_failed",
            Self::InvitationNotFound => "invitation_not_found",
            Self::InvitationAlreadyConsumed => "invitation_already_consumed",
            Self::PersonalContextNotAllowed => "personal_context_not_allowed",
            Self::InternalError => "internal_error",
        }
    }

    /// Human-readable wire `summary`.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AuthRequired => {
                "The request requires authentication and the supplied session is missing or expired."
            }
            Self::PermissionDenied => {
                "The authenticated actor lacks permission to perform this action."
            }
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::InvitationNotFound => {
                "The invitation does not exist or is not visible to the caller."
            }
            Self::InvitationAlreadyConsumed => {
                "The invitation has already been accepted or revoked."
            }
            Self::PersonalContextNotAllowed => {
                "Invitation operations require an organizational context."
            }
            Self::InternalError => "An internal service or store failure occurred.",
        }
    }

    /// Recommended HTTP status for this failure.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AuthRequired => 401,
            Self::PermissionDenied => 403,
            Self::ValidationFailed | Self::PersonalContextNotAllowed => 400,
            Self::InvitationNotFound => 404,
            Self::InvitationAlreadyConsumed => 410,
            Self::InternalError => 500,
        }
    }
}

impl From<InvitationFailureReason> for InvitationFailureCode {
    fn from(reason: InvitationFailureReason) -> Self {
        match reason {
            InvitationFailureReason::AuthRequired => Self::AuthRequired,
            InvitationFailureReason::PermissionDenied => Self::PermissionDenied,
            InvitationFailureReason::ValidationFailed => Self::ValidationFailed,
            InvitationFailureReason::InvitationNotFound => Self::InvitationNotFound,
            InvitationFailureReason::InvitationAlreadyConsumed => Self::InvitationAlreadyConsumed,
            InvitationFailureReason::PersonalContextNotAllowed => Self::PersonalContextNotAllowed,
        }
    }
}

/// Shared `{code, summary}` body for invitation-operation failures.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InvitationFailureBody {
    pub code: InvitationFailureCode,
    pub summary: String,
}

impl InvitationFailureBody {
    #[must_use]
    pub fn from_code(code: InvitationFailureCode) -> Self {
        Self {
            code,
            summary: code.summary().to_owned(),
        }
    }
}
