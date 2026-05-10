//! Identity and Policy subsystem.
//!
//! Owns accounts, organizations, projects, memberships, roles, service
//! accounts, API keys, approval policy, and runtime placement policy. The
//! mechanism for credential verification (local password hashing, OIDC
//! introspection, ...) is deliberately not committed here — R-0001 pins
//! the mechanism behind a [`CredentialVerifier`] impl, with
//! [`Argon2idVerifier`] as the canonical local-password implementation.

mod argon2_verifier;
mod project_identity;
pub mod secret_serde;
mod session_token;

pub use argon2_verifier::Argon2idVerifier;
pub use project_identity::{DesignatedHost, ProjectId, ProviderFamily, RepositoryRef};
pub use session_token::SessionToken;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

const UUID_V7_VERSION: usize = 7;

/// Stable identifier for a Tanren account (`UUIDv7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct AccountId(Uuid);

impl AccountId {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let parsed = Uuid::parse_str(raw).map_err(|_| ValidationError::AccountIdInvalid)?;
        Self::try_from_uuid(parsed)
    }

    pub fn try_from_uuid(value: Uuid) -> Result<Self, ValidationError> {
        if value.get_version_num() != UUID_V7_VERSION {
            return Err(ValidationError::AccountIdInvalid);
        }
        Ok(Self(value))
    }

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

impl<'de> Deserialize<'de> for AccountId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Uuid::deserialize(d)?;
        Self::try_from_uuid(raw).map_err(serde::de::Error::custom)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct OrgId(Uuid);

impl OrgId {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct MembershipId(Uuid);

impl MembershipId {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

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
/// Validated email address. Constructed via [`Email::parse`] which:
/// trims surrounding whitespace, validates against RFC 5322 syntax via
/// the [`email_validator_rfc5322`] crate (RFC 5321 length limits +
/// quoted local parts), additionally requires a TLD-style domain (no
/// dotless or IP-literal domains), and canonicalises to lower-case so
/// case variants of the same address compare equal.
///
/// # Wire-input contract
///
/// `Email` does NOT derive `Deserialize` — the custom impl below routes
/// every wire input through [`parse`](Self::parse). Without this,
/// `#[serde(transparent)]` would let HTTP/MCP/CLI requests carry
/// untrimmed/un-lowercased/RFC-invalid addresses, which would persist
/// verbatim via `Identifier::from_email` and let two case variants of
/// the same logical email register as separate accounts. Codex P1
/// review on PR #133.
///
/// Validation invariants are exercised end-to-end by the @api / @web
/// scenarios in `tests/bdd/features/B-0043-create-account.feature` —
/// case-variant rejection and malformed-email rejection both run
/// through the live wire surface, not through Rust unit tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "email")]
pub struct Email(String);

impl Email {
    /// Parse a raw user-supplied email. Trims, validates against RFC
    /// 5322 + RFC 5321 length limits, and canonicalises to lower-case.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyEmail`] when the input is empty
    /// after trimming. Returns [`ValidationError::InvalidEmail`] when
    /// the input fails RFC 5322 syntax or RFC 5321 length limits.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyEmail);
        }
        email_validator_rfc5322::validate_email(trimmed)
            .map_err(|_| ValidationError::InvalidEmail)?;
        // RFC 5322 permits dotless domains (`user@host`), but the
        // public-internet account flow this type backs always uses a
        // TLD-style domain. Reject dotless and IP-literal domains
        // (`[10.0.0.1]`, `[IPv6:...]`) so identifier collisions and
        // typos are caught at the boundary instead of at the duplicate-
        // identifier error code path. If a future feature needs the
        // permissive RFC 5322 surface, add a sibling
        // `Email::parse_permissive` rather than relaxing here.
        let domain_start = trimmed
            .rfind('@')
            .ok_or(ValidationError::InvalidEmail)?
            .saturating_add(1);
        let domain = &trimmed[domain_start..];
        if domain.starts_with('[') || !domain.contains('.') {
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

impl<'de> Deserialize<'de> for Email {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
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
///
/// `Identifier` does NOT derive `Deserialize` — the custom impl below
/// routes every wire input through [`parse`](Self::parse) so untrimmed
/// or differently-cased identifiers cannot bypass canonicalisation.
/// Validation invariants are exercised end-to-end by the @api / @web
/// scenarios in `tests/bdd/features/B-0043-create-account.feature`
/// (case-variant rejection, malformed-input rejection); per the
/// BDD-only test surface policy there are no Rust unit or doc-tests
/// for these rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
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

impl<'de> Deserialize<'de> for Identifier {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
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
///
/// `InvitationToken` does NOT derive `Deserialize` — the custom impl
/// below routes every wire input through [`parse`](Self::parse) so
/// short-on-wire tokens are rejected at the contract boundary instead
/// of reaching the handler. Validation invariants are exercised
/// end-to-end by the @api / @web scenarios in
/// `tests/bdd/features/B-0043-create-account.feature` (expired-token
/// rejection, missing-token rejection, etc.); per the BDD-only test
/// surface policy there are no Rust unit or doc-tests for these
/// rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
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
    /// token is shorter than 16 bytes (the canonical minimum, kept as
    /// a private `INVITATION_TOKEN_MIN_LEN` constant in this module).
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

impl<'de> Deserialize<'de> for InvitationToken {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for InvitationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A Tanren account.
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

/// A pending invitation with expiry + consumption state.
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

/// An identifier+password credential pair.
#[derive(Debug, Clone)]
pub struct PasswordCredential {
    /// User-facing identifier (email, ...).
    pub identifier: Identifier,
    /// Plaintext password wrapped in `SecretString`.
    pub password: SecretString,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub account: AccountId,
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
    #[error("account id is not a valid uuidv7")]
    AccountIdInvalid,
    #[error("project id is not a valid uuidv7")]
    ProjectIdInvalid,
    #[error("session token is not a valid base64url-no-pad 32-byte secret")]
    SessionTokenInvalid,
    #[error("email is empty")]
    EmptyEmail,
    #[error("email is not in a valid form")]
    InvalidEmail,
    #[error("identifier is empty")]
    EmptyIdentifier,
    #[error("invitation token is empty")]
    InvitationTokenEmpty,
    #[error("invitation token is shorter than the minimum length")]
    InvitationTokenTooShort,
    #[error("repository identity is empty")]
    RepositoryRefEmpty,
    #[error("repository identity is not in canonical owner/name form")]
    RepositoryRefInvalid,
    #[error("provider family is not a valid lowercase slug")]
    ProviderFamilyInvalid,
    #[error("designated host is not in a valid form")]
    DesignatedHostInvalid,
}
