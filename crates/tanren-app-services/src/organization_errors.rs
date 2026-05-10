//! Shared organization failure projection used by interface binaries.

use tanren_contract::{
    AccountFailureReason, CreateOrganizationFailureReason, OrganizationFailureBody,
    OrganizationFailureCode,
};

use crate::AppServiceError;

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
