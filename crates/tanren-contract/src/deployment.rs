//! Deployment posture command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers select or inspect the
//! deployment posture for an account, organization, or installation.
//! They live in `tanren-contract` because every interface binary
//! serialises the same shapes — keeping them here is the architectural
//! guarantee that the surfaces stay equivalent.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgId};
use utoipa::ToSchema;

/// Wire representation of a deployment posture.
///
/// The deployment posture is a top-level decision that determines which
/// capabilities are available. It gates first-run progress to provider
/// selection and ultimately to the first ready project.
///
/// Supported postures are exactly `hosted`, `self_hosted`, and
/// `local_only`. The domain crate (`tanren-domain`) defines a parallel
/// canonical type; this is the wire-shape with schema derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DeploymentPosture {
    /// Tanren operates as a managed hosted service. All capabilities
    /// are available; operational overhead is managed externally.
    Hosted,
    /// Tanren operates as self-hosted infrastructure. All local and
    /// remote capabilities are available; the operator manages
    /// infrastructure and operations.
    SelfHosted,
    /// Tanren operates in a local-only mode. Remote execution, cloud
    /// providers, external secret stores, and team collaboration are
    /// unavailable. Suitable for individual evaluation and development.
    LocalOnly,
}

/// Raw deployment posture input as provided by the caller.
///
/// Uses an unvalidated string so unsupported values survive
/// deserialization and reach shared service validation, which returns
/// [`PostureFailureReason::UnsupportedPosture`] uniformly rather than
/// letting each interface reject unknown values differently at the
/// serde boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct RawPostureInput(String);

impl RawPostureInput {
    /// Construct from a raw string.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the raw input value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Scope for a deployment posture decision.
///
/// The scope identifies the entity the posture applies to. A posture
/// selected at organization scope is inherited by projects under that
/// organization unless explicitly overridden (when policy permits).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeploymentScope {
    /// Account that owns or initiated the posture selection.
    pub account_id: AccountId,
    /// Organization scope, when the posture applies at org level.
    pub org_id: Option<OrgId>,
}

/// Actor attribution for a posture selection event.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActorAttribution {
    /// Account that performed the selection.
    pub actor_id: AccountId,
    /// Wall-clock time at which the selection was made.
    pub selected_at: DateTime<Utc>,
}

/// Request to select a deployment posture.
///
/// The `posture` field carries [`RawPostureInput`] rather than a typed
/// [`DeploymentPosture`] so that unsupported posture values reach shared
/// service validation and produce a uniform `unsupported_posture`
/// failure rather than being rejected differently by each interface's
/// deserialization layer.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PostureSelectionRequest {
    /// Raw posture value supplied by the caller.
    pub posture: RawPostureInput,
    /// Scope for the posture decision.
    pub scope: DeploymentScope,
}

/// Successful response to a posture selection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PostureSelectionResponse {
    /// View of the selected posture and its capabilities.
    pub view: PostureView,
}

/// External-facing view of the active deployment posture for a scope.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PostureView {
    /// The active deployment posture.
    pub posture: DeploymentPosture,
    /// Scope the posture applies to.
    pub scope: DeploymentScope,
    /// Actor and time of the most recent selection.
    pub selected_by: ActorAttribution,
    /// Capability summary for the active posture.
    pub capabilities: CapabilitySummary,
}

/// Summary of capabilities available under a deployment posture.
///
/// Each capability category is either available or listed as
/// unavailable with a user-readable reason explaining why the posture
/// does not support it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CapabilitySummary {
    /// The posture this summary applies to.
    pub posture: DeploymentPosture,
    /// Capability categories available under this posture.
    pub available: Vec<CapabilityCategory>,
    /// Capability categories unavailable under this posture, each with
    /// a user-readable reason.
    pub unavailable: Vec<UnavailableCapability>,
}

/// A category of deployment capability.
///
/// Categories align with subsystem boundaries so capability summaries
/// map cleanly to the architecture's subsystem ownership model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CapabilityCategory {
    /// Container and remote execution targets.
    ExecutionTargets,
    /// Harness adapter support (Codex, Claude Code, `OpenCode`).
    HarnessAdapters,
    /// Remote container and VM provider integrations.
    RemoteProviders,
    /// Organization and team collaboration features.
    TeamCollaboration,
    /// External secret-store adapters (Vault, 1Password, etc.).
    ExternalSecretStores,
    /// Cloud and VM credential types.
    CloudCredentials,
    /// Behavior proof with remote execution.
    RemoteProof,
    /// Webhook and subscription integrations.
    WebhookIntegrations,
    /// Provider integrations (source control, CI, issue trackers).
    ProviderIntegrations,
    /// Service account and API key management.
    ServiceAccounts,
}

/// A capability that is unavailable under the selected posture, with a
/// user-readable explanation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UnavailableCapability {
    /// The unavailable capability category.
    pub category: CapabilityCategory,
    /// User-readable reason the capability is unavailable.
    pub reason: String,
}

/// Execution target class for runtime capability views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TargetClass {
    /// Local container execution.
    LocalContainer,
    /// Remote container execution.
    RemoteContainer,
    /// Remote VM execution.
    RemoteVm,
}

/// Runtime-specific capability view for the selected posture.
///
/// Describes which execution target classes and strategies are available
/// under the active deployment posture.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RuntimeCapabilityView {
    /// The posture this view applies to.
    pub posture: DeploymentPosture,
    /// Execution target classes available under this posture.
    pub available_target_classes: Vec<TargetClass>,
    /// Whether remote execution targets are available.
    pub supports_remote_execution: bool,
    /// Whether parallel execution strategies are available.
    pub supports_parallel_execution: bool,
    /// Maximum concurrent execution targets. `0` means unlimited or
    /// determined by operator configuration.
    pub max_concurrent_targets: u32,
}

/// Credential-specific capability view for the selected posture.
///
/// Describes which credential management features are available under
/// the active deployment posture.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CredentialCapabilityView {
    /// The posture this view applies to.
    pub posture: DeploymentPosture,
    /// Whether external secret-store adapters are available.
    pub supports_external_secret_stores: bool,
    /// Whether cloud and VM credential types are available.
    pub supports_cloud_credentials: bool,
    /// Whether service account and API key management is available.
    pub supports_service_accounts: bool,
    /// Credential kinds available under this posture.
    pub available_credential_kinds: Vec<String>,
}

/// Closed taxonomy of deployment posture operation failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects a
/// `PostureFailureReason` into the same wire shape so callers can match
/// on `code` regardless of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PostureFailureReason {
    /// The requested posture value is not one of the supported postures
    /// (`hosted`, `self_hosted`, `local_only`).
    UnsupportedPosture,
    /// A posture has already been selected for this scope and the
    /// current policy does not allow reselection.
    PostureAlreadySelected,
    /// The authenticated actor does not have permission to change the
    /// deployment posture for this scope.
    PostureChangeDenied,
    /// The supplied scope does not identify a valid account or
    /// organization.
    InvalidScope,
    /// The requested posture is incompatible with existing state in
    /// this scope.
    ScopeConflict,
    /// A posture must be selected before the requested operation can
    /// proceed.
    PostureRequired,
}

impl PostureFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPosture => "unsupported_posture",
            Self::PostureAlreadySelected => "posture_already_selected",
            Self::PostureChangeDenied => "posture_change_denied",
            Self::InvalidScope => "invalid_scope",
            Self::ScopeConflict => "scope_conflict",
            Self::PostureRequired => "posture_required",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UnsupportedPosture => "The requested deployment posture is not supported.",
            Self::PostureAlreadySelected => {
                "A deployment posture has already been selected for this scope."
            }
            Self::PostureChangeDenied => {
                "The authenticated actor does not have permission to change the deployment posture."
            }
            Self::InvalidScope => {
                "The supplied scope does not identify a valid account or organization."
            }
            Self::ScopeConflict => {
                "The requested posture conflicts with existing configuration in this scope."
            }
            Self::PostureRequired => {
                "A deployment posture must be selected before this operation can proceed."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::UnsupportedPosture | Self::InvalidScope | Self::PostureRequired => 400,
            Self::PostureAlreadySelected | Self::ScopeConflict => 409,
            Self::PostureChangeDenied => 403,
        }
    }
}
