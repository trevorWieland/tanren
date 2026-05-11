//! Shared failure-envelope types and mapping helpers.
//!
//! Every interface (api/mcp/cli/tui/web) projects typed failure reasons
//! into the same `{code, summary}` wire shape so callers can match on
//! `code` regardless of transport. The body types and helpers live here
//! — the contract layer — so all surfaces import from one place rather
//! than duplicating stringly constructions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{AccountFailureCode, AccountFailureReason, ProjectFailureCode, ProjectFailureReason};

/// Shared `{code, summary}` account-failure body.
///
/// Every interface that surfaces account-flow errors projects an
/// [`AccountFailureReason`] into this shape so callers can match on
/// `code` regardless of transport.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AccountFailureBody {
    /// Stable error code from the closed taxonomy.
    pub code: AccountFailureCode,
    /// Human-readable summary.
    pub summary: String,
}

impl AccountFailureBody {
    /// Build the canonical body from an [`AccountFailureReason`].
    #[must_use]
    pub fn from_reason(reason: AccountFailureReason) -> Self {
        Self {
            code: AccountFailureCode::from(reason),
            summary: reason.summary().to_owned(),
        }
    }

    /// Build a `validation_failed` body with a custom summary.
    #[must_use]
    pub fn validation_failed(summary: String) -> Self {
        Self {
            code: AccountFailureCode::ValidationFailed,
            summary,
        }
    }

    /// Build the canonical `internal_error` body.
    #[must_use]
    pub fn internal_error() -> Self {
        Self {
            code: AccountFailureCode::InternalError,
            summary: "Tanren encountered an internal error.".to_owned(),
        }
    }
}

/// Shared `{code, summary}` project-failure body.
///
/// Every interface that surfaces project-flow errors projects a
/// [`ProjectFailureReason`] into this shape so callers can match on
/// `code` regardless of transport.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectFailureBody {
    /// Stable error code from the closed taxonomy.
    pub code: ProjectFailureCode,
    /// Human-readable summary.
    pub summary: String,
}

impl ProjectFailureBody {
    /// Build the canonical body from a [`ProjectFailureReason`].
    #[must_use]
    pub fn from_reason(reason: ProjectFailureReason) -> Self {
        Self {
            code: ProjectFailureCode::from(reason),
            summary: reason.summary().to_owned(),
        }
    }

    /// Build a `validation_failed` body with a custom summary.
    #[must_use]
    pub fn validation_failed(summary: String) -> Self {
        Self {
            code: ProjectFailureCode::ValidationFailed,
            summary,
        }
    }

    /// Build the canonical `internal_error` body.
    #[must_use]
    pub fn internal_error() -> Self {
        Self {
            code: ProjectFailureCode::InternalError,
            summary: "Tanren encountered an internal error.".to_owned(),
        }
    }

    /// Build the canonical `auth_required` body with a custom summary.
    #[must_use]
    pub fn auth_required(summary: String) -> Self {
        Self {
            code: ProjectFailureCode::AuthRequired,
            summary,
        }
    }
}
