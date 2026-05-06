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

/// Typed representation of the authenticated actor performing a project
/// action. Constructed by the interface layer from the authenticated session
/// (API/MCP) or from local identity (CLI/TUI). The app-service layer receives
/// this — never a raw `account_id` from a request body.
#[derive(Debug, Clone)]
pub struct ActorContext {
    account_id: AccountId,
    org_ids: Vec<OrgId>,
}

impl ActorContext {
    /// Construct an actor context from an account and the orgs it belongs to.
    #[must_use]
    pub fn new(account_id: AccountId, org_ids: Vec<OrgId>) -> Self {
        Self {
            account_id,
            org_ids,
        }
    }

    /// Construct a minimal actor context with account id only. Used by CLI/TUI
    /// where org membership is resolved by the store at policy-evaluation time.
    #[must_use]
    pub fn from_account_id(account_id: AccountId) -> Self {
        Self {
            account_id,
            org_ids: Vec::new(),
        }
    }

    /// The account performing the action.
    #[must_use]
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    /// The organizations the actor is a member of.
    #[must_use]
    pub fn org_ids(&self) -> &[OrgId] {
        &self.org_ids
    }
}

/// Project lifecycle actions subject to policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectAction {
    /// Connect a repository as a Tanren project.
    Connect,
    /// List projects visible to the actor.
    List,
    /// Disconnect a project from Tanren.
    Disconnect,
    /// Read specs attached to a project.
    Specs,
    /// Read cross-project dependency links for a project.
    Dependencies,
    /// Reconnect a previously disconnected project.
    Reconnect,
}

/// Evaluate whether an actor is permitted to perform a project action.
///
/// Returns [`Decision::Allow`] when the actor's org membership satisfies the
/// action's requirements, or [`Decision::Deny`] otherwise. The `target_org`
/// parameter is required for [`ProjectAction::Connect`]; for other actions
/// the caller should have already resolved visibility through the store.
#[must_use]
pub fn evaluate_project_policy(
    actor: &ActorContext,
    action: ProjectAction,
    target_org: Option<OrgId>,
    actor_can_see_project: bool,
) -> Decision {
    match action {
        ProjectAction::Connect => {
            if let Some(org) = target_org {
                if actor.org_ids.contains(&org) {
                    Decision::Allow
                } else {
                    Decision::Deny(DenialReason::MissingPermission)
                }
            } else {
                Decision::Allow
            }
        }
        ProjectAction::List => Decision::Allow,
        ProjectAction::Disconnect
        | ProjectAction::Specs
        | ProjectAction::Dependencies
        | ProjectAction::Reconnect => {
            if actor_can_see_project {
                Decision::Allow
            } else {
                Decision::Deny(DenialReason::MissingPermission)
            }
        }
    }
}
