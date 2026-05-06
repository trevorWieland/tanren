//! Typed authorization, placement, and budget policy decisions for Tanren.
//!
//! Policy returns typed decisions, never transport-layer errors. The runtime
//! and harness crates do not own policy decisions — they consume them as the
//! [`Decision`] enum below.

use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgId};
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
    /// The actor attempted to target a scope (e.g. organization) they are
    /// not a member of.
    ScopeMismatch,
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

/// Resolved identity context of the actor making a request. Constructed by
/// the interface layer (API session middleware, MCP capability context) from
/// authenticated credentials — never from caller-supplied parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorContext {
    /// The authenticated account issuing the request.
    pub account_id: AccountId,
    /// The organization the account belongs to, if any. Personal accounts
    /// carry `None`.
    pub org: Option<OrgId>,
}

/// The scope a project-registration request targets. Derived from the
/// request's `org` field: `None` maps to [`ScopeTarget::Personal`],
/// `Some(id)` maps to [`ScopeTarget::Org`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeTarget {
    /// Register under the actor's personal account scope.
    Personal,
    /// Register under the specified organization scope.
    Org(OrgId),
}

/// Evaluate whether `actor` is authorized to register a project in `target`
/// scope.
///
/// V1 policy:
/// - Personal scope (`org: None`) is always allowed for the authenticated
///   account.
/// - Organization scope is allowed only when the actor's org matches the
///   requested org.
/// - All other cases are denied with [`DenialReason::ScopeMismatch`].
#[must_use]
pub fn authorize_project_registration(actor: &ActorContext, target: &ScopeTarget) -> Decision {
    match target {
        ScopeTarget::Personal => Decision::Allow,
        ScopeTarget::Org(requested_org) => match actor.org {
            Some(actor_org) if actor_org == *requested_org => Decision::Allow,
            Some(_) | None => Decision::Deny(DenialReason::ScopeMismatch),
        },
    }
}
