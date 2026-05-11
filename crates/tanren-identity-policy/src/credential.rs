//! Account, credential, session, and error types.
//!
//! Groups the identity-primitive structs, the credential verifier trait,
//! and the error enums so that the crate root stays under the line budget
//! while keeping the public re-export surface stable.

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AccountId, Identifier, InvitationToken, OrgId, SessionToken};

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
#[derive(Debug, Clone)]
pub struct PasswordCredential {
    /// User-facing identifier (email, ...).
    pub identifier: Identifier,
    /// Plaintext password — wrapped so accidental `Debug` / `Serialize`
    /// calls do not leak the credential. Hashed before storage by the
    /// `CredentialVerifier`.
    pub password: SecretString,
}

/// A bounded session held by an authenticated account or service identity.
#[derive(Debug, Clone)]
pub struct Session {
    /// The account this session represents.
    pub account: AccountId,
    /// Opaque session token.
    pub token: SessionToken,
}

/// Hashes and verifies a plaintext password against a stored PHC string.
///
/// Mechanism (Argon2id today; potentially OIDC introspection or hardware-
/// backed verifiers later) is the implementor's responsibility. The
/// canonical workspace impl is [`Argon2idVerifier`](crate::Argon2idVerifier).
pub trait CredentialVerifier: Send + Sync + std::fmt::Debug {
    /// Hash a plaintext password into a portable PHC-format string
    /// (`$argon2id$v=19$m=...$<salt>$<hash>`). Salt is generated
    /// internally by the verifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::HashFailed`] when the underlying hashing
    /// primitive raises an error (e.g. invalid parameter combinations).
    fn hash(&self, password: &SecretString) -> Result<String, IdentityError>;

    /// Verify a plaintext password against a stored PHC-format hash.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::InvalidCredential`] when the password
    /// does not match the stored hash, or [`IdentityError::HashFailed`]
    /// when the stored hash string is malformed.
    fn verify(&self, password: &SecretString, stored: &str) -> Result<(), IdentityError>;
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
    /// The hashing primitive raised an error (or the stored hash string
    /// failed to parse). Distinct from
    /// [`IdentityError::InvalidCredential`] which signals a verified
    /// password mismatch.
    #[error("hash error: {0}")]
    HashFailed(String),
    /// User-supplied input failed validation before any verification could run.
    #[error("invalid input: {0}")]
    Validation(#[from] ValidationError),
}

/// Errors raised when constructing a domain newtype from a raw string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ValidationError {
    /// The supplied email string was empty after trimming.
    #[error("email is empty")]
    EmptyEmail,
    /// The supplied email string did not parse as an email address.
    #[error("email is not in a valid form")]
    InvalidEmail,
    /// The supplied identifier was empty after trimming.
    #[error("identifier is empty")]
    EmptyIdentifier,
    /// The supplied invitation token was empty after trimming.
    #[error("invitation token is empty")]
    InvitationTokenEmpty,
    /// The supplied invitation token was shorter than the minimum length.
    #[error("invitation token is shorter than the minimum length")]
    InvitationTokenTooShort,
    #[error("idempotency key is empty")]
    IdempotencyKeyEmpty,
    #[error("idempotency key exceeds the maximum length")]
    IdempotencyKeyTooLong,
    #[error("idempotency key contains control characters")]
    IdempotencyKeyControlCharacter,
    /// The supplied organization name was empty after trimming.
    #[error("organization name is empty")]
    OrganizationNameEmpty,
    /// The supplied organization name did not satisfy naming rules.
    #[error("organization name is malformed")]
    OrganizationNameMalformed,
}
