//! Identity and Policy subsystem.
//!
//! Owns accounts, organizations, projects, memberships, roles, service
//! accounts, API keys, approval policy, and runtime placement policy. The
//! mechanism for credential verification (local password hashing, OIDC
//! introspection, ...) is deliberately not committed here — R-0001 pins
//! the mechanism behind a [`CredentialVerifier`] impl, with
//! [`Argon2idVerifier`] as the canonical local-password implementation.

mod argon2_verifier;
pub mod secret_serde;
mod session_token;

pub use argon2_verifier::Argon2idVerifier;
pub use session_token::SessionToken;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

/// Stable identifier for a Tanren account. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
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

impl From<Uuid> for AccountId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for AccountId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Stable identifier for a Tanren organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
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

impl From<Uuid> for OrgId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for OrgId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for OrgId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Stable identifier for a membership row (links an account to an org).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct MembershipId(Uuid);

impl MembershipId {
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

impl From<Uuid> for MembershipId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for MembershipId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for MembershipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Validated email address. Constructed via [`Email::parse`] which
/// trims surrounding whitespace and lower-cases the string. R-0001
/// keeps the validation deliberately lightweight (presence + single
/// `@`) — full RFC 5322 verification is the responsibility of an
/// out-of-band confirmation flow that lands later.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "email")]
pub struct Email(String);

impl Email {
    /// Parse a raw user-supplied email. Trims and lower-cases.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyEmail`] if the input is empty
    /// after trimming, or [`ValidationError::InvalidEmail`] if the
    /// input does not contain exactly one `@` separating non-empty
    /// local and domain parts.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyEmail);
        }
        let mut parts = trimmed.split('@');
        let local = parts.next().unwrap_or("");
        let domain = parts.next().unwrap_or("");
        if local.is_empty() || domain.is_empty() || parts.next().is_some() {
            return Err(ValidationError::InvalidEmail);
        }
        if !domain.contains('.') {
            return Err(ValidationError::InvalidEmail);
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    /// Borrow the canonical (trimmed + lower-cased) email string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// User-facing identifier for an account. R-0001's chosen mechanism is
/// identifier+password where the identifier is the canonical email; the
/// type wraps the raw string so future mechanisms can lift constraints
/// in one place.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct Identifier(String);

impl Identifier {
    /// Parse a raw identifier string. Trims surrounding whitespace and
    /// lower-cases.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyIdentifier`] if the input is
    /// empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyIdentifier);
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    /// Derive an identifier from a canonical [`Email`]. The identifier
    /// uses the email's canonical form verbatim — it is the user-facing
    /// handle for R-0001.
    #[must_use]
    pub fn from_email(email: &Email) -> Self {
        Self(email.as_str().to_owned())
    }

    /// Borrow the underlying identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Minimum byte length of a valid invitation token.
const INVITATION_TOKEN_MIN_LEN: usize = 16;

/// Opaque invitation token. R-0001 treats the token as a flat string —
/// generation/delivery is R-0005's job; here we just verify and consume.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct InvitationToken(String);

impl InvitationToken {
    /// Parse a raw invitation token.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvitationTokenEmpty`] if the input
    /// is empty after trimming, or
    /// [`ValidationError::InvitationTokenTooShort`] if the trimmed
    /// token is shorter than [`INVITATION_TOKEN_MIN_LEN`] bytes.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::InvitationTokenEmpty);
        }
        if trimmed.len() < INVITATION_TOKEN_MIN_LEN {
            return Err(ValidationError::InvitationTokenTooShort);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the underlying token string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InvitationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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
/// canonical workspace impl is [`Argon2idVerifier`].
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
///
/// Surfaces through `tanren-app-services` as
/// `AccountFailureReason::ValidationFailed` (HTTP 400) — a separate
/// taxonomy from credential failures so callers can distinguish "your
/// inputs are malformed" from "your credentials don't match".
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
}
