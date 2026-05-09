//! Typed authorization, placement, and budget policy decisions for Tanren.
//!
//! Policy returns typed decisions, never transport-layer errors. The runtime
//! and harness crates do not own policy decisions — they consume them as the
//! [`Decision`] enum below.

use serde::{Deserialize, Serialize};
use tanren_contract::{DeploymentPosture, DeploymentPostureCapability};
use tanren_identity_policy::{AccountId, InstallationId, ProjectId};
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

/// Runtime dispatch target checked against deployment posture policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDispatchTarget {
    /// Dispatch work to a local worker.
    Local,
    /// Dispatch work to a remote worker.
    Remote,
}

/// Availability status for a posture-dependent capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CapabilityAvailability {
    /// The capability is allowed by the selected posture.
    Available,
    /// The capability is blocked by the selected posture.
    Unavailable {
        /// Capability that is unavailable.
        capability: DeploymentPostureCapability,
        /// Human-readable explanation suitable for API/MCP/UI output.
        reason: String,
    },
}

/// Scope context used by posture-management policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum DeploymentPosturePolicyScope {
    /// Account scope.
    Account { account_id: AccountId },
    /// Project scope.
    Project { project_id: ProjectId },
    /// Installation scope.
    Installation { installation_id: InstallationId },
}

/// Scope-aware input for deployment-posture management policy checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentPosturePolicyInput {
    /// Account attempting to change posture.
    pub actor: AccountId,
    /// Scope targeted by the request.
    pub scope: DeploymentPosturePolicyScope,
}

/// Evaluate whether an actor may manage deployment posture for a resolved scope.
///
/// Initial model: account owner only. The actor may change posture for their
/// own account scope and is denied for all other scopes until grant models land.
#[must_use]
pub fn evaluate_deployment_posture_management(input: DeploymentPosturePolicyInput) -> Decision {
    match input.scope {
        DeploymentPosturePolicyScope::Account { account_id } if account_id == input.actor => {
            Decision::Allow
        }
        DeploymentPosturePolicyScope::Account { .. }
        | DeploymentPosturePolicyScope::Project { .. }
        | DeploymentPosturePolicyScope::Installation { .. } => {
            Decision::Deny(DenialReason::MissingPermission)
        }
    }
}

/// Evaluate whether an actor may read deployment posture for a resolved scope.
///
/// Initial model: account owner only. The actor may read posture for their own
/// account scope and is denied for all other scopes until grant models land.
#[must_use]
pub fn evaluate_deployment_posture_read(input: DeploymentPosturePolicyInput) -> Decision {
    evaluate_deployment_posture_management(input)
}

/// Check whether runtime dispatch to the selected target is available under
/// the supplied deployment posture.
#[must_use]
pub fn runtime_dispatch_availability(
    posture: DeploymentPosture,
    target: RuntimeDispatchTarget,
) -> CapabilityAvailability {
    let capability = match target {
        RuntimeDispatchTarget::Local => DeploymentPostureCapability::LocalRuntimeDispatch,
        RuntimeDispatchTarget::Remote => DeploymentPostureCapability::RemoteRuntimeDispatch,
    };
    capability_availability(posture, capability)
}

/// Check whether external provider credential integrations are available under
/// the supplied deployment posture.
#[must_use]
pub fn credential_integrations_availability(posture: DeploymentPosture) -> CapabilityAvailability {
    capability_availability(posture, DeploymentPostureCapability::ProviderIntegrations)
}

fn capability_availability(
    posture: DeploymentPosture,
    capability: DeploymentPostureCapability,
) -> CapabilityAvailability {
    let summary = tanren_contract::DeploymentPostureCapabilitySummary::for_posture(posture);
    if summary.available.contains(&capability) {
        CapabilityAvailability::Available
    } else {
        CapabilityAvailability::Unavailable {
            capability,
            reason: format!(
                "Capability '{}' is unavailable when deployment posture is '{}'.",
                capability_name(capability),
                posture.as_wire_value()
            ),
        }
    }
}

const fn capability_name(capability: DeploymentPostureCapability) -> &'static str {
    match capability {
        DeploymentPostureCapability::ManagedControlPlane => "managed_control_plane",
        DeploymentPostureCapability::ProviderIntegrations => "provider_integrations",
        DeploymentPostureCapability::RemoteRuntimeDispatch => "remote_runtime_dispatch",
        DeploymentPostureCapability::LocalRuntimeDispatch => "local_runtime_dispatch",
    }
}
