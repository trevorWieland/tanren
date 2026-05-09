//! Deployment-posture wire shapes.
//!
//! These contracts define the posture vocabulary and scope selectors shared by
//! api/mcp/cli/tui/web. Runtime and provider subsystems consume these same
//! types so posture capability explanations stay centralized.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
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
    /// Canonical ordered list of every supported posture value.
    pub const ALL: [Self; 3] = [Self::Hosted, Self::SelfHosted, Self::LocalOnly];

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

impl fmt::Display for DeploymentPosture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_value())
    }
}

impl FromStr for DeploymentPosture {
    type Err = DeploymentPostureContractFailure;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_wire_value(value.trim())
            .ok_or_else(|| DeploymentPostureContractFailure::unsupported_posture(value.trim()))
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
    /// Posture value to set.
    pub posture: DeploymentPosture,
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
    /// Canonical ordered list of every capability exposed by posture
    /// summaries.
    pub const ALL: [Self; 4] = [
        Self::ManagedControlPlane,
        Self::ProviderIntegrations,
        Self::RemoteRuntimeDispatch,
        Self::LocalRuntimeDispatch,
    ];

    /// Stable `snake_case` wire value for this capability.
    #[must_use]
    pub const fn as_wire_value(self) -> &'static str {
        match self {
            Self::ManagedControlPlane => "managed_control_plane",
            Self::ProviderIntegrations => "provider_integrations",
            Self::RemoteRuntimeDispatch => "remote_runtime_dispatch",
            Self::LocalRuntimeDispatch => "local_runtime_dispatch",
        }
    }

    /// Parse a capability from its wire value.
    #[must_use]
    pub fn from_wire_value(value: &str) -> Option<Self> {
        match value {
            "managed_control_plane" => Some(Self::ManagedControlPlane),
            "provider_integrations" => Some(Self::ProviderIntegrations),
            "remote_runtime_dispatch" => Some(Self::RemoteRuntimeDispatch),
            "local_runtime_dispatch" => Some(Self::LocalRuntimeDispatch),
            _ => None,
        }
    }
}

impl fmt::Display for DeploymentPostureCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire_value())
    }
}

impl FromStr for DeploymentPostureCapability {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_wire_value(value.trim()).ok_or(())
    }
}

/// Canonical unavailable-reason taxonomy for posture capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentPostureCapabilityUnavailableReason {
    /// The selected posture does not run Tanren-managed control-plane
    /// services.
    RequiresManagedControlPlane,
    /// The selected posture does not allow provider integrations.
    RequiresProviderIntegrations,
    /// The selected posture does not allow non-local runtime dispatch.
    RequiresRemoteRuntimeDispatch,
    /// The selected posture does not allow local runtime dispatch.
    RequiresLocalRuntimeDispatch,
}

impl DeploymentPostureCapabilityUnavailableReason {
    const fn for_capability(
        capability: DeploymentPostureCapability,
    ) -> DeploymentPostureCapabilityUnavailableReason {
        match capability {
            DeploymentPostureCapability::ManagedControlPlane => Self::RequiresManagedControlPlane,
            DeploymentPostureCapability::ProviderIntegrations => Self::RequiresProviderIntegrations,
            DeploymentPostureCapability::RemoteRuntimeDispatch => {
                Self::RequiresRemoteRuntimeDispatch
            }
            DeploymentPostureCapability::LocalRuntimeDispatch => Self::RequiresLocalRuntimeDispatch,
        }
    }

    /// Stable wire summary for cross-interface rendering.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::RequiresManagedControlPlane => {
                "This posture does not include Tanren-managed control plane services."
            }
            Self::RequiresProviderIntegrations => {
                "This posture does not include provider integration capabilities."
            }
            Self::RequiresRemoteRuntimeDispatch => {
                "This posture does not include remote runtime dispatch."
            }
            Self::RequiresLocalRuntimeDispatch => {
                "This posture does not include local runtime dispatch."
            }
        }
    }
}

/// Unavailable capability and its canonical explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentPostureUnavailableCapability {
    /// Capability unavailable for the selected posture.
    pub capability: DeploymentPostureCapability,
    /// Canonical reason for the unavailable capability.
    pub reason: DeploymentPostureCapabilityUnavailableReason,
}

/// Explanation of which capabilities are enabled for a selected posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentPostureCapabilitySummary {
    /// Capabilities available in the posture.
    pub available: Vec<DeploymentPostureCapability>,
    /// Capabilities unavailable in the posture.
    pub unavailable: Vec<DeploymentPostureUnavailableCapability>,
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
                unavailable.push(DeploymentPostureUnavailableCapability {
                    capability,
                    reason: DeploymentPostureCapabilityUnavailableReason::for_capability(
                        capability,
                    ),
                });
            }
        }
        Self {
            available,
            unavailable,
        }
    }
}

/// Supported posture option surfaced by read/list operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SupportedDeploymentPosture {
    /// Canonical posture value.
    pub posture: DeploymentPosture,
    /// Canonical capability explanation for this posture.
    pub capability_summary: DeploymentPostureCapabilitySummary,
}

/// Read model for all supported posture options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SupportedDeploymentPosturesResponse {
    /// Supported posture options.
    pub supported: Vec<SupportedDeploymentPosture>,
}

/// Read model for a scope's current deployment posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentPostureReadModel {
    /// Scope where the posture is now in effect.
    pub scope: DeploymentPostureScope,
    /// Persisted posture.
    pub posture: DeploymentPosture,
    /// Canonical capability explanation for the posture.
    pub capability_summary: DeploymentPostureCapabilitySummary,
}

/// Response shape for reading the current posture for a scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CurrentDeploymentPostureResponse {
    /// Current selection for the requested scope, when recorded.
    pub current: Option<DeploymentPostureReadModel>,
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

/// Canonical rendered failure payload for posture interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentPostureFailureBody {
    /// Stable machine-readable failure code.
    pub code: String,
    /// User-readable summary.
    pub summary: String,
}

/// Closed taxonomy for posture-flow failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeploymentPostureFailureReason {
    /// The provided posture wire value is not supported.
    UnsupportedPosture,
    /// Authenticated actor does not have permission to access posture.
    PermissionDenied,
    /// The requested scope identifier does not exist.
    ScopeNotFound,
    /// A contract-level validation rule failed.
    ValidationFailed,
    /// The service is temporarily unavailable.
    Unavailable,
    /// An unexpected internal error occurred.
    InternalError,
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
            Self::Unavailable => "unavailable",
            Self::InternalError => "internal_error",
        }
    }

    /// Parse a failure reason from a wire code.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "unsupported_posture" => Some(Self::UnsupportedPosture),
            "permission_denied" => Some(Self::PermissionDenied),
            "scope_not_found" => Some(Self::ScopeNotFound),
            "validation_failed" => Some(Self::ValidationFailed),
            "unavailable" => Some(Self::Unavailable),
            "internal_error" => Some(Self::InternalError),
            _ => None,
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
                "The authenticated actor does not have permission to access deployment posture for this scope."
            }
            Self::ScopeNotFound => "The requested posture scope does not exist.",
            Self::ValidationFailed => {
                "The submitted posture request did not satisfy contract-level validation."
            }
            Self::Unavailable => "Tanren is temporarily unavailable. Please try again shortly.",
            Self::InternalError => "Tanren encountered an internal error.",
        }
    }

    /// Recommended HTTP status for this failure over API/MCP transports.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::UnsupportedPosture | Self::ValidationFailed => 400,
            Self::PermissionDenied => 403,
            Self::ScopeNotFound => 404,
            Self::Unavailable => 503,
            Self::InternalError => 500,
        }
    }

    /// Render this failure reason as a wire body with optional detail.
    ///
    /// Empty detail values fall back to the canonical default summary.
    #[must_use]
    pub fn render(self, detail: Option<&str>) -> DeploymentPostureFailureBody {
        let summary = detail
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(|| self.summary().to_owned(), str::to_owned);
        DeploymentPostureFailureBody {
            code: self.code().to_owned(),
            summary,
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

    /// Render this contract failure into the canonical `{code, summary}`
    /// wire payload.
    #[must_use]
    pub fn render(&self) -> DeploymentPostureFailureBody {
        self.reason.render(Some(&self.detail))
    }
}
