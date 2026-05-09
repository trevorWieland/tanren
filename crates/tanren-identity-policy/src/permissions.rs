//! Permission-oriented identity/policy primitives shared across interfaces.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Stable identifier for a Tanren project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ProjectId(Uuid);

impl ProjectId {
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

impl From<Uuid> for ProjectId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for ProjectId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Canonical permission identifier used by policy and interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct PermissionName(String);

impl PermissionName {
    /// Parse and validate a raw permission identifier.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ValidationError::EmptyPermissionName`] when the
    /// value is empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, crate::ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::ValidationError::EmptyPermissionName);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Wrap a normalized permission name.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying permission name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PermissionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PermissionName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Name of a role template used as the source of a permission grant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct RoleTemplateName(String);

impl RoleTemplateName {
    /// Parse and validate a role-template identifier.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ValidationError::EmptyRoleTemplateName`] when the
    /// value is empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, crate::ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::ValidationError::EmptyRoleTemplateName);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Wrap a role-template name.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying role-template name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoleTemplateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RoleTemplateName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Effective runtime state for a permission after policy has been applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionEffectiveState {
    /// Permission is effective and usable.
    Granted,
    /// Permission was granted but constrained by policy.
    Constrained,
}

/// How a permission grant entered the actor's authorization set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PermissionGrantSource {
    /// Explicit one-off grant.
    Direct,
    /// Grant inherited from applying a role template.
    RoleTemplate {
        /// Template that produced this grant.
        role_template: RoleTemplateName,
    },
}

/// Human-readable reason describing why policy constrained a grant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct PolicyConstraintReason(String);

impl PolicyConstraintReason {
    /// Parse and validate a policy-constraint reason.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ValidationError::EmptyPolicyConstraintReason`] when the
    /// value is empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, crate::ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::ValidationError::EmptyPolicyConstraintReason);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Wrap a policy-constraint reason string.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the underlying reason.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PolicyConstraintReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PolicyConstraintReason {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Policy scope that produced a constraint on a permission grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolicyConstraintSource {
    /// Constraint came from organization policy.
    OrganizationPolicy,
    /// Constraint came from project policy.
    ProjectPolicy,
}
