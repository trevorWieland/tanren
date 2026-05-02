//! Identity and Policy subsystem.
//!
//! Owns accounts, organizations, projects, memberships, roles, service
//! accounts, API keys, approval policy, and runtime placement policy. The
//! mechanism for credential verification (local password hashing, OIDC
//! introspection, ...) is deliberately not committed here — R-0001 onwards
//! pin the mechanism behind a [`CredentialVerifier`] impl.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Stable identifier for a Tanren account. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(Uuid);

impl AccountId {
    /// Wrap a raw UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Allocate a fresh time-ordered id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// The underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Stable identifier for a Tanren organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrgId(Uuid);

impl OrgId {
    /// Wrap a raw UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Allocate a fresh time-ordered id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// The underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// User-facing identifier for an account. R-0001's chosen mechanism is
/// identifier+password where the identifier is an email; the type wraps
/// the raw string so future mechanisms can lift constraints in one place.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identifier(String);

impl Identifier {
    /// Construct from a raw string (already lower-cased + trimmed by the
    /// caller for identifier+password mechanisms).
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque invitation token. R-0001 treats the token as a flat string —
/// generation/delivery is R-0005's job; here we just verify and consume.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InvitationToken(String);

impl InvitationToken {
    /// Wrap a raw token string.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying token string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Tanren account. `org` is `None` for self-signed-up personal accounts;
/// invitation-based accounts carry the inviting `OrgId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Stable id.
    pub id: AccountId,
    /// User-facing identifier (email, ...).
    pub identifier: Identifier,
    /// Wall-clock time the account was created.
    pub created_at: DateTime<Utc>,
    /// Owning organization — `None` for personal accounts (self-signup).
    pub org: Option<OrgId>,
}

/// A pending invitation seeded by R-0005's invite flow (or by
/// `tanren-testkit` fixtures during R-0001 BDD). Carries the invitee's
/// destination organization plus expiry / consumption state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invitation {
    /// The opaque token shared with the invitee out-of-band.
    pub token: InvitationToken,
    /// Organization the new account joins on acceptance.
    pub inviting_org: OrgId,
    /// Expiry instant — tokens older than this are rejected.
    pub expires_at: DateTime<Utc>,
    /// Set when the invitation has been accepted (or revoked).
    pub consumed_at: Option<DateTime<Utc>>,
}

/// An identifier+password credential pair as supplied by the caller.
/// Hashing is the responsibility of the [`CredentialVerifier`] impl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordCredential {
    /// User-facing identifier (email, ...).
    pub identifier: Identifier,
    /// Plaintext password — hashed before storage / verified against a stored hash.
    pub password: String,
}

/// A bounded session held by an authenticated account or service identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// The account this session represents.
    pub account: AccountId,
    /// Opaque session token.
    pub token: String,
}

/// Verifies a credential and returns a [`Session`] on success. Mechanism
/// (local password, OIDC, ...) is the implementor's responsibility.
pub trait CredentialVerifier: Send + Sync {
    /// Verify the supplied credential and produce a session.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::InvalidCredential`] if the credential does
    /// not verify.
    fn verify(&self, credential: &PasswordCredential) -> Result<Session, IdentityError>;
}

/// Errors raised by identity-policy operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IdentityError {
    /// An account with the supplied identifier already exists.
    #[error("an account already exists for the supplied identifier")]
    DuplicateIdentifier,
    /// The supplied credential did not verify (or did not match a known account).
    #[error("the supplied credential is invalid")]
    InvalidCredential,
    /// No invitation matched the supplied token.
    #[error("no invitation matches the supplied token")]
    InvitationNotFound,
    /// The invitation token has expired.
    #[error("the invitation has expired")]
    InvitationExpired,
    /// The invitation has already been consumed (or revoked).
    #[error("the invitation has already been consumed")]
    InvitationAlreadyConsumed,
}
