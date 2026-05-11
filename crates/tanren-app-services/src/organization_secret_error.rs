//! Errors raised by organization-secret service handlers.
//!
//! Maps to the shared organization-secret taxonomy without leaking
//! whether hidden cross-org secrets exist beyond authorized visibility.
//! Every variant maps to a stable wire code so interface layers can
//! present consistent error bodies.

use tanren_store::StoreError;
use thiserror::Error;

/// Errors raised by organization-secret service handlers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OrganizationSecretServiceError {
    /// Request requires authentication; no valid session supplied.
    #[error("authentication required")]
    AuthRequired,
    /// Authenticated actor lacks permission for the operation.
    #[error("permission denied")]
    PermissionDenied,
    /// User-supplied input failed validation.
    #[error("validation failed: {0}")]
    ValidationFailed(String),
    /// Referenced secret does not exist or is outside authorized visibility.
    #[error("secret not found")]
    NotFound,
    /// Operation conflicts with current state (e.g., duplicate name).
    #[error("conflict")]
    Conflict,
    /// The underlying store layer raised an error.
    #[error(transparent)]
    Store(#[from] StoreError),
}
