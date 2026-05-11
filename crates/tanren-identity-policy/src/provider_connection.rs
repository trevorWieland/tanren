//! Provider connection domain types — identifiers and provider kind enum.
//!
//! Split out of `lib.rs` to stay under the workspace per-file line budget.
//! Follows the same newtype pattern as `AccountId`, `OrgId`, etc.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Stable identifier for a provider connection record. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ProviderConnectionId(Uuid);

impl ProviderConnectionId {
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

impl From<Uuid> for ProviderConnectionId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for ProviderConnectionId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for ProviderConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The kind of provider a connection targets.
///
/// Each variant corresponds to an integration capability in the
/// provider-integrations subsystem. The closed set is enforced at the
/// database level by a CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
pub enum ProviderKind {
    /// Source-control provider (GitHub, GitLab, etc.).
    SourceControl,
    /// Identity provider (OIDC, SAML, etc.).
    Identity,
}

impl ProviderKind {
    /// Stored string representation used in the database.
    #[must_use]
    pub const fn as_stored_value(self) -> &'static str {
        match self {
            Self::SourceControl => "source_control",
            Self::Identity => "identity",
        }
    }

    /// Parse a stored value back into a `ProviderKind`.
    #[must_use]
    pub fn from_stored_value(value: &str) -> Option<Self> {
        match value {
            "source_control" => Some(Self::SourceControl),
            "identity" => Some(Self::Identity),
            _ => None,
        }
    }
}
