//! Credential lifecycle status transition registry.

use crate::{ConfigurationValidationFailure, UserCredentialStatus};

/// Closed registry of allowed credential status transitions.
///
/// `(from, to)` pairs. `None` marks the initial status for newly created credentials.
const CREDENTIAL_STATUS_TRANSITIONS: &[(Option<UserCredentialStatus>, UserCredentialStatus)] = &[
    (None, UserCredentialStatus::Pending),
    (
        Some(UserCredentialStatus::Pending),
        UserCredentialStatus::Pending,
    ),
    (
        Some(UserCredentialStatus::Active),
        UserCredentialStatus::Pending,
    ),
    (
        Some(UserCredentialStatus::Invalid),
        UserCredentialStatus::Pending,
    ),
];

/// Validate the initial status for a newly created credential.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::InvalidStatusTransition`] when the
/// target status is not an allowed initial status.
pub fn validate_credential_create_status(
    target: UserCredentialStatus,
) -> Result<UserCredentialStatus, ConfigurationValidationFailure> {
    if CREDENTIAL_STATUS_TRANSITIONS.contains(&(None, target)) {
        Ok(target)
    } else {
        Err(ConfigurationValidationFailure::InvalidStatusTransition {
            from: "none".to_owned(),
            to: status_label(target),
        })
    }
}

/// Validate that a credential status transition is allowed by the closed registry.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::InvalidStatusTransition`] when the
/// `(from, to)` pair is not part of the registry.
pub fn validate_credential_status_transition(
    from: UserCredentialStatus,
    target: UserCredentialStatus,
) -> Result<UserCredentialStatus, ConfigurationValidationFailure> {
    if CREDENTIAL_STATUS_TRANSITIONS.contains(&(Some(from), target)) {
        Ok(target)
    } else {
        Err(ConfigurationValidationFailure::InvalidStatusTransition {
            from: status_label(from),
            to: status_label(target),
        })
    }
}

fn status_label(status: UserCredentialStatus) -> String {
    match status {
        UserCredentialStatus::Pending => "pending".to_owned(),
        UserCredentialStatus::Active => "active".to_owned(),
        UserCredentialStatus::Invalid => "invalid".to_owned(),
    }
}
