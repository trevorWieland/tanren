//! Identity and Policy subsystem.
//!
//! Owns accounts, organizations, projects, memberships, roles, service
//! accounts, API keys, approval policy, and runtime placement policy. The
//! mechanism for credential verification (local password hashing, OIDC
//! introspection, ...) is deliberately not committed here — R-0001 onwards
//! pin the mechanism behind a [`CredentialVerifier`] impl.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable identifier for a Tanren account.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(String);

impl AccountId {
    /// Wrap a raw id string.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A bounded session held by an authenticated account or service identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// The account or service id this session represents.
    pub account: AccountId,
    /// Opaque session token.
    pub token: String,
}

/// Verifies a credential and returns a [`Session`] on success. Mechanism
/// (local password, OIDC, ...) is the implementor's responsibility; R-0001
/// onwards introduces concrete impls.
pub trait CredentialVerifier: Send + Sync {
    /// Verify the supplied credential and produce a session. Returns
    /// [`IdentityError::Rejected`] if the credential does not verify.
    fn verify(&self, credential: &str) -> Result<Session, IdentityError>;
}

/// Errors raised by identity-policy operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IdentityError {
    /// The credential did not verify.
    #[error("credential rejected")]
    Rejected,
}
