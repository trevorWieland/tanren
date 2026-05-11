//! Provider connection domain types.
//!
//! Types for the provider-integrations subsystem: connection identity,
//! capability classification, credential redaction, connection status,
//! and reachable-resource summaries. These types are pure domain
//! values — no I/O, no infrastructure dependencies. [`RedactedToken`]
//! intentionally limits secret access to the port layer.

use schemars::JsonSchema;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::DomainError;

// ---------------------------------------------------------------------------
// ProviderConnectionId — variable-length newtype
// ---------------------------------------------------------------------------

/// Stable identifier for a provider connection record.
///
/// Variable-length newtype wrapping a `String` slug (e.g.
/// `"identity-github-01"`, `"scm-gitlab-main"`). Unlike UUID newtypes,
/// this uses [`ProviderConnectionId::parse`] / [`ProviderConnectionId::as_str`]
/// per the variable-length newtype recipe.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ProviderConnectionId(String);

impl ProviderConnectionId {
    /// Validate and construct a provider connection id.
    ///
    /// The raw value must be non-empty and contain only lowercase
    /// alphanumeric characters, hyphens, and underscores.
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(DomainError::InvariantViolation(
                "provider connection id must not be empty".into(),
            ));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(DomainError::InvariantViolation(
                "provider connection id must contain only lowercase alphanumeric, hyphens, and underscores".into(),
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The string form of this identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// ProviderKind — capability-based classification
// ---------------------------------------------------------------------------

/// The capability class a provider connection serves.
///
/// Mirrors the provider-integrations architecture: `Identity` maps to
/// the `identity_provider` capability (external login, org identity,
/// group claims, authorization refresh); `SourceControl` maps to the
/// `source_control` capability (repositories, branches, commits, PRs,
/// reviews, merge execution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Identity provider: external login, org identity, group claims,
    /// authorization refresh mechanics.
    Identity,
    /// Source-control provider: repository access, branches, commits,
    /// pull requests, reviews, mergeability, merge execution.
    SourceControl,
}

// ---------------------------------------------------------------------------
// RedactedToken — opaque credential wrapper
// ---------------------------------------------------------------------------

/// Opaque wrapper around a provider credential token.
///
/// `Debug` redacts; `Display` and `Serialize` are deliberately absent
/// so the token cannot leak through logging, API responses, or
/// serialization. The only access point is [`RedactedToken::expose_secret`],
/// which the port layer calls to make outbound provider API requests.
/// Application-facing read models and projections never receive the
/// raw secret — core invariant 7 ("credentials are use-only").
#[derive(Clone)]
pub struct RedactedToken(SecretString);

impl RedactedToken {
    /// Wrap a raw secret string into a redacted token.
    #[must_use]
    pub fn from_secret(secret: SecretString) -> Self {
        Self(secret)
    }

    /// Expose the inner secret string.
    ///
    /// Only the port/adapter layer should call this — it uses the
    /// credential to make outbound provider API requests. Application
    /// read models, projections, and API responses must never call this
    /// (core invariant 7: "credentials are use-only").
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for RedactedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RedactedToken(<redacted>)")
    }
}

// ---------------------------------------------------------------------------
// ConnectionStatus — connection lifecycle state
// ---------------------------------------------------------------------------

/// The current state of a provider connection.
///
/// `Connected` means the provider is reachable and credentials are
/// valid. `Failed` carries a human-readable reason describing the
/// failure mode (auth expired, rate-limited, provider unreachable,
/// etc.) per core invariant 10 ("Provider failure is recoverable").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    /// Provider is reachable and credentials are valid.
    Connected,
    /// Connection has failed. The reason describes the failure mode
    /// (e.g. "authorization expired", "rate limited", "provider
    /// unreachable").
    Failed {
        /// Human-readable description of the failure mode.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// ReachableRepository — source-control resource summary
// ---------------------------------------------------------------------------

/// A source-control repository discovered through a provider connection.
///
/// Plain data with no token fields — credentials are use-only (core
/// invariant 7) and never appear in resource summaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ReachableRepository {
    /// Provider-scoped identifier for the repository (e.g. "owner/repo").
    pub identifier: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Whether the connected credentials grant write access.
    pub writable: bool,
}

// ---------------------------------------------------------------------------
// ReachableIdentity — identity resource summary
// ---------------------------------------------------------------------------

/// An identity (user, group, or organization) discovered through a
/// provider connection.
///
/// Plain data with no token fields — credentials are use-only (core
/// invariant 7) and never appear in resource summaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ReachableIdentity {
    /// Provider-scoped identifier for the identity (e.g. username, org
    /// slug).
    pub identifier: String,
    /// Human-readable display name.
    pub display_name: String,
    /// The kind of identity discovered.
    pub kind: IdentityKind,
}

/// The kind of identity a [`ReachableIdentity`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    /// An individual user account.
    User,
    /// A group within the provider.
    Group,
    /// An organization or tenant.
    Organization,
}
