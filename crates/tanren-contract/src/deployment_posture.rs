//! Deployment-posture wire shapes.
//!
//! These contracts define the posture vocabulary and scope selectors shared by
//! api/mcp/cli/tui/web. Runtime and provider subsystems consume these same
//! types so posture capability explanations stay centralized.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, InstallationId, ProjectId};
use utoipa::ToSchema;

/// Closed deployment posture set used on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentPosture {
    /// Tanren-managed hosted posture.
    Hosted,
    /// Customer-managed self-hosted posture.
    SelfHosted,
    /// Local-only posture with no hosted runtime dependency.
    LocalOnly,
}

impl DeploymentPosture {
    /// Stable `snake_case` wire value for this posture.
    #[must_use]
    pub const fn as_wire_value(self) -> &'static str {
        match self {
            Self::Hosted => "hosted",
            Self::SelfHosted => "self_hosted",
            Self::LocalOnly => "local_only",
        }
    }

    /// Parse a posture from its wire value.
    #[must_use]
    pub fn from_wire_value(value: &str) -> Option<Self> {
        match value {
            "hosted" => Some(Self::Hosted),
            "self_hosted" => Some(Self::SelfHosted),
            "local_only" => Some(Self::LocalOnly),
            _ => None,
        }
    }

    const fn supports(self, capability: DeploymentPostureCapability) -> bool {
        matches!(
            (self, capability),
            (
                Self::Hosted,
                DeploymentPostureCapability::ManagedControlPlane
                    | DeploymentPostureCapability::ProviderIntegrations
                    | DeploymentPostureCapability::RemoteRuntimeDispatch
            ) | (
                Self::SelfHosted,
                DeploymentPostureCapability::ProviderIntegrations
                    | DeploymentPostureCapability::RemoteRuntimeDispatch
                    | DeploymentPostureCapability::LocalRuntimeDispatch
            ) | (
                Self::LocalOnly,
                DeploymentPostureCapability::LocalRuntimeDispatch
            )
        )
    }
}

/// Scope selector for where a deployment posture applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum DeploymentPostureScope {
    /// Account-wide posture.
    Account { account_id: AccountId },
    /// Project-specific posture.
    Project { project_id: ProjectId },
    /// Installation-wide posture.
    Installation { installation_id: InstallationId },
}

/// Command shape for choosing a deployment posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetDeploymentPostureRequest {
    /// Scope the posture change targets.
    pub scope: DeploymentPostureScope,
    /// Raw wire posture value. Must match one of
    /// `hosted | self_hosted | local_only`.
    pub posture: String,
}

impl SetDeploymentPostureRequest {
    /// Validate the wire posture value into the closed posture set.
    pub fn posture(&self) -> Result<DeploymentPosture, DeploymentPostureContractFailure> {
        DeploymentPosture::from_wire_value(self.posture.trim()).ok_or_else(|| {
            DeploymentPostureContractFailure::unsupported_posture(self.posture.trim())
        })
    }
}

/// Capability vocabulary explained alongside each posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentPostureCapability {
    /// Tanren-hosted control-plane services.
    ManagedControlPlane,
    /// External provider credentials and capability integrations.
    ProviderIntegrations,
    /// Dispatching runtime work to non-local workers.
    RemoteRuntimeDispatch,
    /// Dispatching runtime work to a local worker.
    LocalRuntimeDispatch,
}

impl DeploymentPostureCapability {
    const ALL: [Self; 4] = [
        Self::ManagedControlPlane,
        Self::ProviderIntegrations,
        Self::RemoteRuntimeDispatch,
        Self::LocalRuntimeDispatch,
    ];
}

/// Explanation of which capabilities are enabled for a selected posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentPostureCapabilitySummary {
    /// Capabilities available in the posture.
    pub available: Vec<DeploymentPostureCapability>,
    /// Capabilities unavailable in the posture.
    pub unavailable: Vec<DeploymentPostureCapability>,
}

impl DeploymentPostureCapabilitySummary {
    /// Build the canonical capability explanation for a posture.
    #[must_use]
    pub fn for_posture(posture: DeploymentPosture) -> Self {
        let mut available = Vec::new();
        let mut unavailable = Vec::new();
        for capability in DeploymentPostureCapability::ALL {
            if posture.supports(capability) {
                available.push(capability);
            } else {
                unavailable.push(capability);
            }
        }
        Self {
            available,
            unavailable,
        }
    }
}

/// Response shape after recording a posture selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetDeploymentPostureResponse {
    /// Scope where the posture is now in effect.
    pub scope: DeploymentPostureScope,
    /// Persisted posture.
    pub posture: DeploymentPosture,
    /// Canonical capability explanation for the posture.
    pub capability_summary: DeploymentPostureCapabilitySummary,
}

/// Closed taxonomy for posture-flow failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeploymentPostureFailureReason {
    /// The provided posture wire value is not supported.
    UnsupportedPosture,
    /// Authenticated actor does not have permission to change posture.
    PermissionDenied,
    /// The requested scope identifier does not exist.
    ScopeNotFound,
    /// A contract-level validation rule failed.
    ValidationFailed,
}

impl DeploymentPostureFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPosture => "unsupported_posture",
            Self::PermissionDenied => "permission_denied",
            Self::ScopeNotFound => "scope_not_found",
            Self::ValidationFailed => "validation_failed",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UnsupportedPosture => {
                "The provided deployment posture is unsupported. Use hosted, self_hosted, or local_only."
            }
            Self::PermissionDenied => {
                "The authenticated actor does not have permission to change deployment posture for this scope."
            }
            Self::ScopeNotFound => "The requested posture scope does not exist.",
            Self::ValidationFailed => {
                "The submitted posture request did not satisfy contract-level validation."
            }
        }
    }

    /// Recommended HTTP status for this failure over API/MCP transports.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::UnsupportedPosture | Self::ValidationFailed => 400,
            Self::PermissionDenied => 403,
            Self::ScopeNotFound => 404,
        }
    }
}

/// User-readable failure payload for deployment-posture requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentPostureContractFailure {
    /// Machine-readable failure taxonomy value.
    pub reason: DeploymentPostureFailureReason,
    /// Human-readable detail suitable for end users.
    pub detail: String,
}

impl DeploymentPostureContractFailure {
    /// Build an unsupported-posture contract failure with the invalid
    /// user input included in the explanation.
    #[must_use]
    pub fn unsupported_posture(input: &str) -> Self {
        let detail = format!(
            "Unsupported deployment posture '{input}'. Supported values: hosted, self_hosted, local_only."
        );
        Self {
            reason: DeploymentPostureFailureReason::UnsupportedPosture,
            detail,
        }
    }
}
