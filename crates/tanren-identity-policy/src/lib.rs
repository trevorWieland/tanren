//! Identity and Policy subsystem.
//!
//! Owns accounts, organizations, projects, memberships, roles, service
//! accounts, API keys, approval policy, and runtime placement policy. The
//! mechanism for credential verification (local password hashing, OIDC introspection, ...)
//! is deliberately not committed here — R-0001 pins the mechanism behind a
//! [`CredentialVerifier`] impl, with [`Argon2idVerifier`] as the canonical
//! local-password implementation.

mod approval_policy;
mod argon2_verifier;
mod credential;
mod organization;
pub mod secret_serde;
mod session_token;

pub use approval_policy::{
    ApprovalAuthority, ApprovalRequirement, ApprovalRule, GatedAction, ScopedPermissionDecision,
    ScopedPermissionSet, evaluate_set_policy_gate,
};
pub use argon2_verifier::Argon2idVerifier;
pub use credential::{
    Account, CredentialVerifier, IdentityError, Invitation, PasswordCredential, Session,
    ValidationError,
};
pub use organization::{
    IdempotencyKey, OrganizationCapability, OrganizationName, OrganizationPermission,
    OrganizationPermissionDecision, OrganizationPermissionGate, ParseOrganizationPermissionError,
    evaluate_organization_permission_gate, organization_capability,
};
pub use session_token::SessionToken;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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

/// Stable identifier for an approval policy record. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ApprovalPolicyId(Uuid);

impl ApprovalPolicyId {
    /// Allocate a fresh time-ordered id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wrap a pre-existing UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Extract the inner UUID.
    #[must_use]
    pub const fn into_inner(self) -> Uuid {
        self.0
    }

    /// Parse a UUID string into an [`ApprovalPolicyId`].
    ///
    /// # Errors
    ///
    /// Returns `Err` when the input is not a valid UUID.
    pub fn parse(input: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(input).map(Self)
    }
}

impl From<Uuid> for ApprovalPolicyId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for ApprovalPolicyId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for ApprovalPolicyId {
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
/// Validation invariants are exercised end-to-end by the @api / @web scenarios in
/// `tests/bdd/features/B-0043-create-account.feature`, not through Rust unit tests.
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

/// Opaque invitation token. R-0001 treats the token as a flat string
/// because the cryptographic shape is the responsibility of the
/// invitation subsystem (R-0005). Identity-policy only validates length
/// and non-emptiness.
///
/// `InvitationToken` does NOT derive `Deserialize` — the custom impl
/// below routes every wire input through [`parse`](Self::parse) so
/// blank / under-length tokens are rejected at the boundary.
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
    /// token is shorter than a private `INVITATION_TOKEN_MIN_LEN`
    /// constant in this module).
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

    /// Borrow the token.
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
