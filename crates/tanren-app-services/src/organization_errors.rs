//! Shared organization failure projection used by interface binaries.

use tanren_contract::{
    AccountFailureReason, CreateOrganizationFailureReason, OrganizationFailureBody,
    OrganizationFailureCode, OrganizationSecretFailureBody, OrganizationSecretFailureReason,
};

use crate::AppServiceError;
use crate::OrganizationSecretServiceError;

/// Transport-ready projection for organization operation failures.
#[derive(Debug, Clone)]
pub struct OrganizationErrorProjection {
    /// Recommended HTTP status for the failure.
    pub http_status: u16,
    /// Shared `{code, summary}` failure body.
    pub body: OrganizationFailureBody,
}

/// Project an [`AppServiceError`] into the shared organization
/// `{code, summary}` taxonomy used by API/MCP/CLI/web projections.
#[must_use]
pub fn map_organization_error(err: &AppServiceError) -> OrganizationErrorProjection {
    match err {
        AppServiceError::Account(AccountFailureReason::AuthRequired) => organization_failure(
            OrganizationFailureCode::AuthRequired,
            AccountFailureReason::AuthRequired.summary(),
        ),
        AppServiceError::Account(AccountFailureReason::PermissionDenied) => organization_failure(
            OrganizationFailureCode::PermissionDenied,
            AccountFailureReason::PermissionDenied.summary(),
        ),
        AppServiceError::CreateOrganization(CreateOrganizationFailureReason::DuplicateName) => {
            organization_failure(
                OrganizationFailureCode::Conflict,
                CreateOrganizationFailureReason::DuplicateName.summary(),
            )
        }
        AppServiceError::CreateOrganization(
            CreateOrganizationFailureReason::IdempotencyConflict,
        ) => organization_failure(
            OrganizationFailureCode::IdempotencyConflict,
            CreateOrganizationFailureReason::IdempotencyConflict.summary(),
        ),
        AppServiceError::InvalidInput(message) => {
            organization_failure(OrganizationFailureCode::ValidationFailed, message)
        }
        AppServiceError::Store(_) | AppServiceError::Account(_) => organization_failure(
            OrganizationFailureCode::InternalError,
            "Tanren encountered an internal error.",
        ),
    }
}

/// Project an [`OrganizationSecretServiceError`] into the shared
/// organization-secret `{code, summary}` taxonomy used by
/// API/MCP/CLI/web projections. Does not leak whether hidden cross-org
/// secrets exist beyond authorized visibility — `NotFound` and store
/// errors both map to the same wire code to prevent enumeration.
#[must_use]
pub fn map_organization_secret_error(
    err: &OrganizationSecretServiceError,
) -> OrganizationSecretFailureBody {
    match err {
        OrganizationSecretServiceError::AuthRequired => OrganizationSecretFailureBody::from_reason(
            OrganizationSecretFailureReason::AuthRequired,
        ),
        OrganizationSecretServiceError::PermissionDenied => {
            OrganizationSecretFailureBody::from_reason(
                OrganizationSecretFailureReason::PermissionDenied,
            )
        }
        OrganizationSecretServiceError::ValidationFailed(msg) => OrganizationSecretFailureBody {
            code: OrganizationSecretFailureReason::ValidationFailed
                .code()
                .to_owned(),
            summary: msg.clone(),
            reason: OrganizationSecretFailureReason::ValidationFailed,
        },
        // NotFound and Store errors map to the same codes intentionally:
        // callers cannot distinguish "does not exist" from "internal error"
        // to prevent cross-org secret enumeration.
        OrganizationSecretServiceError::NotFound => {
            OrganizationSecretFailureBody::from_reason(OrganizationSecretFailureReason::NotFound)
        }
        OrganizationSecretServiceError::Conflict => {
            OrganizationSecretFailureBody::from_reason(OrganizationSecretFailureReason::Conflict)
        }
        OrganizationSecretServiceError::Store(_) => OrganizationSecretFailureBody::from_reason(
            OrganizationSecretFailureReason::InternalError,
        ),
    }
}

fn organization_failure(
    code: OrganizationFailureCode,
    summary: impl Into<String>,
) -> OrganizationErrorProjection {
    OrganizationErrorProjection {
        http_status: code.http_status(),
        body: OrganizationFailureBody {
            code,
            summary: summary.into(),
        },
    }
}
