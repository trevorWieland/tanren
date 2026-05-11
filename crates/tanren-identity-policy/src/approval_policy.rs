//! Domain types for organization approval policies.
//!
//! An approval policy gates a `GatedAction` behind a required number of
//! approvals from members holding a specific permission. Each
//! organization may define at most one policy per action.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::ValidationError;

/// Stable identifier for an approval policy row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ApprovalPolicyId(Uuid);

impl ApprovalPolicyId {
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

impl From<Uuid> for ApprovalPolicyId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for ApprovalPolicyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Maximum length for a gated action string.
const GATED_ACTION_MAX_CHARS: usize = 128;

/// A gated action name — identifies the operation that requires approval.
///
/// The string is trimmed, lower-cased, and validated for length and
/// charset (ASCII alphanumeric, underscore, and dot) so it serves as a
/// stable key within an organization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct GatedAction(String);

impl GatedAction {
    /// Parse a raw gated action name.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::GatedActionEmpty`] if empty after
    /// trimming, or [`ValidationError::GatedActionMalformed`] if the
    /// normalized form exceeds 128 chars or contains non-ASCII-
    /// alphanumeric/underscore/dot characters.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::GatedActionEmpty);
        }
        let normalized = trimmed.to_ascii_lowercase();
        if normalized.chars().count() > GATED_ACTION_MAX_CHARS {
            return Err(ValidationError::GatedActionMalformed);
        }
        let valid = normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.');
        if !valid {
            return Err(ValidationError::GatedActionMalformed);
        }
        Ok(Self(normalized))
    }

    /// Borrow the normalized action key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for GatedAction {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for GatedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A parsed approval-policy rule — the domain envelope returned by the
/// store layer. Carries the full persisted state of an approval gate:
/// which action is gated, how many approvals are required, which
/// permission approvers must hold, and the optimistic-concurrency
/// version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRule {
    /// Stable policy row id.
    pub id: ApprovalPolicyId,
    /// Organization that owns this policy.
    pub org_id: crate::OrgId,
    /// The action gated by this policy.
    pub gated_action: GatedAction,
    /// Number of distinct approvals required (> 0).
    pub required_approvals: u16,
    /// Permission an approver must hold to satisfy this gate.
    pub permitted_approver_permission: String,
    /// Optimistic-concurrency version — incremented on each update.
    pub version: u16,
}
