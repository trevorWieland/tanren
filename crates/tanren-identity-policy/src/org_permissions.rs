//! Organization-level permissions newtype.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::ValidationError;

/// Organization-level permissions for a membership or invitation.
/// Represents the permission level a member holds within an organization.
/// Stored as a string tag (e.g. `"member"`, `"admin"`); the taxonomy is
/// owned by `tanren-identity-policy` and extended by later behavior slices.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrgPermissions(String);

impl OrgPermissions {
    /// The default membership permission level.
    #[must_use]
    pub fn member() -> Self {
        Self("member".to_owned())
    }

    /// Organization administrator.
    #[must_use]
    pub fn admin() -> Self {
        Self("admin".to_owned())
    }

    /// Parse a raw permission string. Rejects empty or whitespace-only
    /// values.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyOrgPermissions`] if the input is
    /// empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyOrgPermissions);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the underlying permission string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OrgPermissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
