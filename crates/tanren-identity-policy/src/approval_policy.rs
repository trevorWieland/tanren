//! Approval-policy domain types and permission gates.
//!
//! Defines the domain-authoritative types for organization approval
//! policies: the actions that can be gated, the rules that gate them,
//! the authority that qualifies approvers, and the evaluator that decides
//! whether a caller may set policy within a specific organisation scope.

use std::fmt;
use std::num::NonZeroU8;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    ApprovalPolicyId, OrgId, OrganizationPermission, OrganizationPermissionDecision,
    evaluate_organization_permission_gate,
};

// ---------------------------------------------------------------------------
// GatedAction
// ---------------------------------------------------------------------------

/// Actions that can be gated by an approval policy.
///
/// The set is intentionally non-exhaustive: future policy actions will be
/// added without a semver break. The initial variants cover organisation-level
/// destructive or sensitive operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum GatedAction {
    /// Revoking a credential (API key, service-account credential, …).
    RevokeCredential,
    /// Deleting a project and all associated data.
    DeleteProject,
    /// Changing organisation-level policy settings.
    ChangeOrgPolicy,
}

impl GatedAction {
    /// Stable kebab-case wire key for this gated action.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RevokeCredential => "revoke-credential",
            Self::DeleteProject => "delete-project",
            Self::ChangeOrgPolicy => "change-org-policy",
        }
    }
}

impl fmt::Display for GatedAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for GatedAction {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "revoke-credential" => Ok(Self::RevokeCredential),
            "delete-project" => Ok(Self::DeleteProject),
            "change-org-policy" => Ok(Self::ChangeOrgPolicy),
            _ => Err("unknown gated action"),
        }
    }
}

// ---------------------------------------------------------------------------
// ApprovalRequirement
// ---------------------------------------------------------------------------

/// Whether a gated action requires approval and how many approvals.
///
/// This is the domain-level view-projection enum; the contract crate
/// mirrors it as its wire-shape [`ApprovalRequirement`]. The variant set
/// covers the full range of approval semantics:
///
/// - [`None`](Self::None): no approval needed — the action proceeds
///   immediately. Returned when a [`GatedAction`] has **no** matching
///   [`ApprovalRule`].
/// - [`SingleApproval`](Self::SingleApproval): any single designated
///   approver suffices. Projected from an [`ApprovalRule`] whose
///   `required_approvals` is 1.
/// - [`MajorityApproval`](Self::MajorityApproval): a majority of
///   designated approvers must approve. Projected from an [`ApprovalRule`]
///   whose `required_approvals` is ≥ 2 but < total eligible approvers.
/// - [`UnanimousApproval`](Self::UnanimousApproval): all designated
///   approvers must approve. Projected by the caller when
///   `required_approvals` equals the total eligible approver pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequirement {
    /// No approval needed — the action proceeds immediately.
    None,
    /// Any single designated approver suffices.
    SingleApproval,
    /// A majority of designated approvers must approve.
    MajorityApproval,
    /// All designated approvers must approve.
    UnanimousApproval,
}

impl ApprovalRequirement {
    /// Stable kebab-case wire key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SingleApproval => "single-approval",
            Self::MajorityApproval => "majority-approval",
            Self::UnanimousApproval => "unanimous-approval",
        }
    }
}

impl fmt::Display for ApprovalRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// ApprovalAuthority
// ---------------------------------------------------------------------------

/// Describes who is authorised to approve a gated action.
///
/// `ApprovalAuthority` captures the actor attribute that qualifies someone
/// as an approver. The initial authority model is permission-based: an
/// approver must hold a specific organisation permission. Future authority
/// models (role-based, named-approver, etc.) will extend this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ApprovalAuthority {
    /// The organisation permission that qualifies an approver.
    pub permission: OrganizationPermission,
}

impl ApprovalAuthority {
    /// Create a permission-based approval authority.
    #[must_use]
    pub const fn from_permission(permission: OrganizationPermission) -> Self {
        Self { permission }
    }
}

// ---------------------------------------------------------------------------
// ApprovalRule
// ---------------------------------------------------------------------------

/// A single rule within an approval policy.
///
/// An `ApprovalRule` is scoped to a specific [`OrgId`] and binds a
/// [`GatedAction`] to its approval threshold ([`NonZeroU8`]) and the
/// [`OrganizationPermission`] that approvers must satisfy.
/// `required_approvals` is `NonZeroU8` so zero-approver policies are
/// unconstructible at the type level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRule {
    /// The organization this rule belongs to.
    pub org_id: OrgId,
    /// The policy this rule belongs to.
    pub policy_id: ApprovalPolicyId,
    /// The action this rule gates.
    pub gated_action: GatedAction,
    /// How many approvals are required. `NonZeroU8` guarantees at least one.
    pub required_approvals: NonZeroU8,
    /// The permission an approver must hold to satisfy this rule.
    pub permitted_approver_permission: OrganizationPermission,
}

impl ApprovalRule {
    /// Project this rule into an [`ApprovalRequirement`] for read-model views.
    ///
    /// Maps the numeric `required_approvals` to the categorical
    /// [`ApprovalRequirement`] enum used by the contract layer. With the
    /// current single-threshold model the mapping is:
    ///
    /// | `required_approvals` | `ApprovalRequirement` |
    /// |---|---|
    /// | 1 | `SingleApproval` |
    /// | 2.. | `MajorityApproval` |
    ///
    /// The projection intentionally never returns `UnanimousApproval` —
    /// that variant requires the caller to know the total eligible
    /// approver pool size, which is not stored on the rule itself. The
    /// caller should compare `required_approvals` against the pool size
    /// and override to `UnanimousApproval` when they match. The `None`
    /// variant is returned by the absence of a matching rule rather than
    /// by projection from an existing rule (since `NonZeroU8` prevents a
    /// zero-approver rule).
    #[must_use]
    pub fn to_requirement(&self) -> ApprovalRequirement {
        if self.required_approvals.get() == 1 {
            ApprovalRequirement::SingleApproval
        } else {
            ApprovalRequirement::MajorityApproval
        }
    }
}

// ---------------------------------------------------------------------------
// ScopedPermissionSet
// ---------------------------------------------------------------------------

/// An actor's resolved permission set, **already scoped** to a specific
/// [`OrgId`] at construction time.
///
/// `ScopedPermissionSet` exists so that callers cannot pass permission
/// evidence from one organisation into a gate evaluated for a different
/// organisation. The only way to obtain a `ScopedPermissionSet` is via
/// [`ScopedPermissionSet::new`], which requires the caller to name the
/// [`OrgId`] the permissions were resolved for. Gate functions consume
/// this type and the scope is carried through into the resulting
/// [`ScopedPermissionDecision`], making cross-organisation confusion a
/// compile-time impossibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedPermissionSet {
    /// The organisation these permissions were resolved for.
    scope: OrgId,
    /// Permissions the actor holds in `scope`.
    permissions: Vec<OrganizationPermission>,
}

impl ScopedPermissionSet {
    /// Construct a scoped permission set.
    ///
    /// Callers — typically the store or app-service layer that has just
    /// loaded the actor's grants for a given organisation — pass the
    /// [`OrgId`] and the resolved permission slice. The set is frozen
    /// after construction.
    #[must_use]
    pub fn new(scope: OrgId, permissions: &[OrganizationPermission]) -> Self {
        Self {
            scope,
            permissions: permissions.to_vec(),
        }
    }

    /// The organisation scope these permissions are valid for.
    #[must_use]
    pub const fn scope(&self) -> OrgId {
        self.scope
    }

    /// Whether the actor holds a specific permission in this scope.
    #[must_use]
    pub fn contains(&self, perm: OrganizationPermission) -> bool {
        self.permissions.contains(&perm)
    }
}

// ---------------------------------------------------------------------------
// ScopedPermissionDecision
// ---------------------------------------------------------------------------

/// An [`OrganizationPermissionDecision`] irreversibly tied to the
/// [`OrgId`] scope it was evaluated against.
///
/// Both fields are private so that consumers cannot destructure or
/// directly read the decision. Use [`scope`](Self::scope) to read the
/// originating organisation, and [`is_allowed_for`](Self::is_allowed_for)
/// to check the decision against a *target* organisation — which returns
/// `false` if the scopes do not match, preventing cross-scope application.
/// No accessor exposes the raw [`OrganizationPermissionDecision`], so
/// cross-organisation misuse is a compile-time impossibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopedPermissionDecision {
    /// The organisation scope this decision was evaluated for.
    scope: OrgId,
    /// The allow/deny decision.
    decision: OrganizationPermissionDecision,
}

impl ScopedPermissionDecision {
    /// The organisation scope this decision was evaluated for.
    #[must_use]
    pub const fn scope(self) -> OrgId {
        self.scope
    }

    /// Returns `true` when the decision is
    /// [`OrganizationPermissionDecision::Allow`] **and** `target_org`
    /// matches the [`OrgId`] this decision was evaluated for.
    ///
    /// When the scopes do not match, returns `false` — the decision is
    /// not valid for the target organisation.
    #[must_use]
    pub fn is_allowed_for(self, target_org: OrgId) -> bool {
        if self.scope != target_org {
            return false;
        }
        matches!(self.decision, OrganizationPermissionDecision::Allow)
    }
}

// ---------------------------------------------------------------------------
// Gate evaluator
// ---------------------------------------------------------------------------

/// Evaluate whether the actor may set organisation approval policy.
///
/// Takes a [`ScopedPermissionSet`] whose permissions are already bound
/// to a specific [`OrgId`] and the target [`OrgId`] the caller intends
/// to mutate. The function checks that the permission set's scope
/// matches `target_scope` — if they differ the decision is
/// [`OrganizationPermissionDecision::Deny`] regardless of the actor's
/// permissions — then checks whether the set contains
/// [`OrganizationPermission::SetPolicy`].
///
/// The returned [`ScopedPermissionDecision`] carries both the allow/deny
/// outcome and the originating scope. It can only be checked via
/// [`ScopedPermissionDecision::is_allowed_for`], which requires the
/// consumer to name the organisation they intend to apply the decision
/// to — preventing cross-organisation misuse at compile time.
#[must_use]
pub fn evaluate_set_policy_gate(
    actor_perms: &ScopedPermissionSet,
    target_scope: OrgId,
) -> ScopedPermissionDecision {
    // Scope mismatch: the permission set was resolved for a different
    // organisation. Deny unconditionally — the actor's permissions in
    // one organisation never authorise mutations in another.
    if actor_perms.scope() != target_scope {
        return ScopedPermissionDecision {
            scope: actor_perms.scope(),
            decision: OrganizationPermissionDecision::Deny,
        };
    }
    let allowed = actor_perms.contains(OrganizationPermission::SetPolicy);
    ScopedPermissionDecision {
        scope: actor_perms.scope(),
        decision: evaluate_organization_permission_gate(allowed),
    }
}
