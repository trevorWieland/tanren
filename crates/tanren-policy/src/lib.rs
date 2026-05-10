//! Typed authorization, placement, and budget policy decisions for Tanren.
//!
//! Policy returns typed decisions, never transport-layer errors. The runtime
//! and harness crates do not own policy decisions — they consume them as the
//! [`Decision`] enum below.

use serde::{Deserialize, Serialize};
use tanren_identity_policy::OrganizationPermission;
use thiserror::Error;

/// The outcome of evaluating a policy against an actor and a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// The policy permits the requested action.
    Allow,
    /// The policy denies the requested action. The reason is carried as a
    /// [`DenialReason`] so callers can surface a typed cause without leaking
    /// internal policy state.
    Deny(DenialReason),
}

/// Why a policy returned [`Decision::Deny`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DenialReason {
    /// The actor does not hold a permission required by the resource.
    MissingPermission,
    /// A scoped quota or budget has been exhausted.
    QuotaExhausted,
    /// The runtime placement constraints could not be satisfied.
    PlacementUnsatisfiable,
}

/// Errors raised when policy evaluation itself cannot complete (distinct from
/// a deliberate [`Decision::Deny`]).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PolicyError {
    /// Required policy inputs were missing or malformed.
    #[error("policy evaluation failed: missing input '{0}'")]
    MissingInput(String),
}

/// Stable capability metadata for organization-scoped permission gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrganizationCapability {
    /// Stable capability id surfaced to interface layers.
    pub key: &'static str,
    /// Human-readable capability summary.
    pub summary: &'static str,
}

/// Permission gate definition owned by policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrganizationPermissionGate {
    /// Capability metadata for this gate.
    pub capability: OrganizationCapability,
    /// Backing organization permission required by the capability.
    pub permission: OrganizationPermission,
}

impl OrganizationPermissionGate {
    /// Build a policy-owned gate from an organization permission.
    #[must_use]
    pub const fn from_permission(permission: OrganizationPermission) -> Self {
        Self {
            capability: organization_capability(permission),
            permission,
        }
    }
}

/// Map an organization permission to stable capability metadata.
#[must_use]
pub const fn organization_capability(permission: OrganizationPermission) -> OrganizationCapability {
    match permission {
        OrganizationPermission::Invite => OrganizationCapability {
            key: "organization.invite",
            summary: "Invite people to the organization",
        },
        OrganizationPermission::ManageAccess => OrganizationCapability {
            key: "organization.manage_access",
            summary: "Manage organization access for members",
        },
        OrganizationPermission::Configure => OrganizationCapability {
            key: "organization.configure",
            summary: "Configure organization-level defaults",
        },
        OrganizationPermission::SetPolicy => OrganizationCapability {
            key: "organization.set_policy",
            summary: "Manage organization policy settings",
        },
        OrganizationPermission::Delete => OrganizationCapability {
            key: "organization.delete",
            summary: "Delete the organization",
        },
    }
}

/// Evaluate a permission gate from a concrete store permission result.
#[must_use]
pub const fn evaluate_organization_permission_gate(allowed: bool) -> Decision {
    if allowed {
        Decision::Allow
    } else {
        Decision::Deny(DenialReason::MissingPermission)
    }
}
