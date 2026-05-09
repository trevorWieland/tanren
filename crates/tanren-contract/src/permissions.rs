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

/// Request payload for self-permission introspection.
///
/// The shape is intentionally empty because identity comes from the current
/// authenticated session, not from caller-supplied target ids.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MyPermissionsRequest;

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
