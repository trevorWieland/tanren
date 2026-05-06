//! Project command/response wire shapes.
//!
//! Shapes for connecting, listing, disconnecting, and reconnecting Tanren
//! projects. Cross-project dependency signalling is included here so the
//! M-0007 lookup layer can emit the unresolved-link signal through the same
//! contract surface.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, ProviderConnectionId, SpecId};
use utoipa::ToSchema;

/// External-facing view of a spec attached to a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SpecView {
    /// Stable spec id.
    pub id: SpecId,
    /// Owning project.
    pub project_id: ProjectId,
    /// Human-readable title.
    pub title: String,
    /// Wall-clock creation time.
    pub created_at: DateTime<Utc>,
}

/// Response for listing specs attached to a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectSpecsResponse {
    /// Specs attached to the project.
    pub specs: Vec<SpecView>,
}

/// External-facing view of a cross-project dependency link annotated with
/// resolution status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DependencyView {
    /// Project that owns the dependency reference.
    pub source_project_id: ProjectId,
    /// Spec within the source project carrying the reference.
    pub source_spec_id: SpecId,
    /// Target project of the dependency.
    pub target_project_id: ProjectId,
    /// Whether the dependency is resolved.
    pub resolved: bool,
    /// When the link was detected.
    pub detected_at: DateTime<Utc>,
}

/// Response for listing cross-project dependency links for a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectDependenciesResponse {
    /// Cross-project dependency links.
    pub dependencies: Vec<DependencyView>,
}

/// Parameter type for the `project.list` MCP tool and the API
/// `GET /projects` query string.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct ListProjectsParams {
    /// Account whose projects to list. Ignored by the API (derived from
    /// session). Used by the MCP for backward compatibility during the
    /// transition to session-based auth.
    #[serde(default)]
    pub account_id: Option<AccountId>,
}

/// Parameter type for the `project.specs` and `project.dependencies` MCP
/// tools.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ProjectIdParams {
    /// Project to query.
    pub project_id: ProjectId,
    /// Account performing the query. Required for policy evaluation.
    #[serde(default)]
    pub actor_account_id: Option<AccountId>,
}

/// Request body for `POST /projects/{id}/disconnect`.
#[derive(Debug, Deserialize, JsonSchema, ToSchema)]
pub struct DisconnectProjectBody {
    /// Account requesting the disconnect. Ignored by the API (derived from
    /// session). Retained for backward wire compatibility.
    #[serde(default)]
    pub account_id: Option<AccountId>,
}

/// Request body for `POST /projects/{id}/reconnect`.
#[derive(Debug, Deserialize, JsonSchema, ToSchema)]
pub struct ReconnectProjectBody {
    /// Account requesting the reconnect. Ignored by the API (derived from
    /// session). Retained for backward wire compatibility.
    #[serde(default)]
    pub account_id: Option<AccountId>,
}

/// Wire body for project-flow failures. Follows the shared `{code, summary}`
/// taxonomy from `docs/architecture/subsystems/interfaces.md`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectFailureBody {
    /// Stable error code from the closed taxonomy.
    pub code: String,
    /// Human-readable summary.
    pub summary: String,
}

impl ProjectFailureBody {
    /// Construct a failure body from a [`ProjectFailureReason`].
    #[must_use]
    pub fn from_reason(reason: ProjectFailureReason) -> Self {
        Self {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        }
    }
}

/// Request to connect an existing repository as a Tanren project.
/// References a configured source-control provider connection and
/// repository resource — never a raw authority-bearing URL.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectRequest {
    /// Account initiating the connection. Ignored by all interfaces —
    /// identity is derived from the typed [`ActorContext`] passed to the
    /// handler. Retained for backward wire compatibility.
    #[serde(default)]
    pub account_id: Option<AccountId>,
    /// Organization that will own the project.
    pub org_id: OrgId,
    /// Human-readable name for the project.
    pub name: String,
    /// Configured source-control provider connection to resolve the
    /// repository through.
    pub provider_connection_id: ProviderConnectionId,
    /// Opaque repository resource identifier within the provider
    /// connection (e.g. `"acme/tanren-app"` for a GitHub connection,
    /// a local name for the fixture provider).
    pub resource_id: String,
}

/// Successful project-connection response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectResponse {
    /// View of the newly connected project.
    pub project: ProjectView,
}

/// Response listing the projects accessible to the caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListProjectsResponse {
    /// Projects visible to the authenticated account.
    pub projects: Vec<ProjectView>,
}

/// Request to disconnect a project from Tanren.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DisconnectProjectRequest {
    /// Project to disconnect.
    pub project_id: ProjectId,
    /// Account requesting the disconnect. Ignored by all interfaces —
    /// identity is derived from the typed [`ActorContext`] passed to the
    /// handler. Retained for backward wire compatibility.
    #[serde(default)]
    pub account_id: Option<AccountId>,
}

/// Successful project-disconnect response.
///
/// Carries the disconnected project id, the post-disconnect account project
/// view (so callers can verify the project is absent), and any inbound
/// cross-project dependency links that are now unresolved because their
/// target was just disconnected.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DisconnectProjectResponse {
    /// The project that was disconnected.
    pub project_id: ProjectId,
    /// Projects still visible to the requesting account after the
    /// disconnect — the disconnected project is absent from this list.
    pub account_projects: Vec<ProjectView>,
    /// Inbound cross-project dependencies that are now unresolved because
    /// their target project was just disconnected.
    pub unresolved_inbound_dependencies: Vec<ProjectDependencyResponse>,
}

/// Cross-project dependency signal emitted when a dependency points into a
/// disconnected (or unknown) project. M-0007 owns the lookup; this is the
/// wire shape the signal travels on.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectDependencyResponse {
    /// The project that owns the dependency reference.
    pub source_project_id: ProjectId,
    /// The spec within the source project carrying the reference.
    pub source_spec_id: SpecId,
    /// The target project id that could not be resolved.
    pub unresolved_target_project_id: ProjectId,
    /// Wall-clock time at which the unresolved link was detected.
    pub detected_at: DateTime<Utc>,
}

/// Successful project-reconnection response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReconnectProjectResponse {
    /// View of the reconnected project, with prior specs restored.
    pub project: ProjectView,
}

/// External-facing view of a Tanren project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectView {
    /// Stable project id.
    pub id: ProjectId,
    /// Human-readable name.
    pub name: String,
    /// Owning organization.
    pub org_id: OrgId,
    /// Redacted display reference for the connected repository (e.g.
    /// `github.com/acme/tanren-app`, `local://bdd-temp`). Never contains
    /// credentials or secret-bearing URLs.
    pub display_ref: String,
    /// Wall-clock time the project was originally connected.
    pub connected_at: DateTime<Utc>,
    /// Wall-clock time the project was disconnected, if applicable.
    pub disconnected_at: Option<DateTime<Utc>>,
}

/// Closed taxonomy of project-flow failures.
///
/// Follows the same `{code, summary}` pattern as
/// [`AccountFailureReason`](crate::AccountFailureReason). Every interface
/// (api/mcp/cli/tui/web) projects a `ProjectFailureReason` into the same
/// wire shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectFailureReason {
    /// An active implementation loop exists on the project — disconnect is
    /// blocked until it completes or is cancelled. Enforcement is M-0011's
    /// responsibility; at M-0003 time the precondition is fixtured.
    ActiveLoopExists,
    /// No project matches the supplied identifier.
    ProjectNotFound,
    /// The underlying repository is unreachable or has been deleted.
    RepositoryUnavailable,
    /// User-supplied input failed validation before any verification could run.
    ValidationFailed,
    /// The authenticated actor is not authorized to perform the requested
    /// action on the project. Returned when the policy layer denies access
    /// based on org membership or project visibility.
    Unauthorized,
    /// No configured source-control provider connection matches the
    /// supplied identifier.
    ProviderConnectionNotFound,
    /// The provider connection exists but lacks the capabilities required
    /// to connect a project (read access, merge permission assessment).
    InsufficientProviderCapabilities,
}

impl ProjectFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ActiveLoopExists => "active_loop_exists",
            Self::ProjectNotFound => "project_not_found",
            Self::RepositoryUnavailable => "repository_unavailable",
            Self::ValidationFailed => "validation_failed",
            Self::Unauthorized => "unauthorized",
            Self::ProviderConnectionNotFound => "provider_connection_not_found",
            Self::InsufficientProviderCapabilities => "insufficient_provider_capabilities",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::ActiveLoopExists => {
                "An active implementation loop exists on the project and must be resolved before disconnect."
            }
            Self::ProjectNotFound => "No project matches the supplied identifier.",
            Self::RepositoryUnavailable => {
                "The underlying repository is unreachable or has been removed."
            }
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::Unauthorized => {
                "The authenticated actor is not authorized to perform this action."
            }
            Self::ProviderConnectionNotFound => {
                "No configured source-control provider connection matches the supplied identifier."
            }
            Self::InsufficientProviderCapabilities => {
                "The provider connection lacks the capabilities required to connect a project."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces. Centralized so every transport reports the
    /// same status for the same failure code.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::ProjectNotFound => 404,
            Self::ActiveLoopExists
            | Self::RepositoryUnavailable
            | Self::InsufficientProviderCapabilities => 409,
            Self::ValidationFailed | Self::ProviderConnectionNotFound => 400,
            Self::Unauthorized => 403,
        }
    }
}
