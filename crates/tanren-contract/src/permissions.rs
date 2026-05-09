//! Self-permission query wire shapes.
//!
//! These contract types model the read-only "my permissions" surface used
//! by api/mcp/cli/tui/web. The request is intentionally self-scoped: callers
//! cannot pass an arbitrary account id.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    OrgId, PermissionEffectiveState, PermissionGrantSource, PermissionName, PolicyConstraintReason,
    PolicyConstraintSource, ProjectId,
};
use utoipa::ToSchema;

/// Default number of permission entries returned by the self-permissions view
/// when callers do not provide a limit.
pub const MY_PERMISSIONS_DEFAULT_LIMIT: u16 = 100;
/// Maximum allowed `limit` for self-permissions reads across all interfaces.
pub const MY_PERMISSIONS_MAX_LIMIT: u16 = 200;

/// Request payload for self-permission introspection.
///
/// Identity comes from the current authenticated session; callers may only
/// provide pagination hints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsRequest {
    /// Maximum number of permission entries to return.
    ///
    /// Values above [`MY_PERMISSIONS_MAX_LIMIT`] are clamped to that maximum.
    /// Missing or zero values fall back to [`MY_PERMISSIONS_DEFAULT_LIMIT`].
    pub limit: Option<u16>,
}

impl Default for MyPermissionsRequest {
    fn default() -> Self {
        Self {
            limit: Some(MY_PERMISSIONS_DEFAULT_LIMIT),
        }
    }
}

impl MyPermissionsRequest {
    /// Resolve the caller-supplied limit to a bounded value.
    #[must_use]
    pub fn resolved_limit(&self) -> u16 {
        match self.limit {
            Some(0) | None => MY_PERMISSIONS_DEFAULT_LIMIT,
            Some(limit) => limit.min(MY_PERMISSIONS_MAX_LIMIT),
        }
    }
}

/// Response payload for self-permission introspection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsResponse {
    /// Organization-scoped permission sections visible to the caller.
    pub organizations: Vec<MyOrganizationPermissions>,
    /// Project-scoped permission sections visible to the caller.
    pub projects: Vec<MyProjectPermissions>,
}

/// Organization-level permission section for the current caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyOrganizationPermissions {
    /// Organization these effective permissions are scoped to.
    pub org_id: OrgId,
    /// Effective permission entries for this organization.
    pub permissions: Vec<MyPermissionEntry>,
}

/// Project-level permission section for the current caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyProjectPermissions {
    /// Project these effective permissions are scoped to.
    pub project_id: ProjectId,
    /// Effective permission entries for this project.
    pub permissions: Vec<MyPermissionEntry>,
}

/// One effective permission entry shown in the self-introspection view.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionEntry {
    /// Canonical permission identifier.
    pub permission: PermissionName,
    /// Effective state after policy is applied.
    pub effective_state: PermissionEffectiveState,
    /// How this permission was granted.
    pub grant_source: PermissionGrantSource,
    /// Why policy constrained this permission, when applicable.
    pub policy_constraint: Option<PermissionConstraintView>,
}

/// Optional policy-constraint detail for a permission entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PermissionConstraintView {
    /// Human-readable reason associated with the constraint.
    pub reason: PolicyConstraintReason,
    /// Scope that produced the constraint.
    pub source: PolicyConstraintSource,
}
