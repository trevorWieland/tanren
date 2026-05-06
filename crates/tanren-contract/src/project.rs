//! Project command/response wire shapes.
//!
//! Shapes for connecting, listing, disconnecting, and reconnecting Tanren
//! projects. Cross-project dependency signalling is included here so the
//! M-0007 lookup layer can emit the unresolved-link signal through the same
//! contract surface.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, SpecId};
use utoipa::ToSchema;

/// Request to connect an existing repository as a Tanren project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectRequest {
    /// Account initiating the connection.
    pub account_id: AccountId,
    /// Organization that will own the project.
    pub org_id: OrgId,
    /// Human-readable name for the project.
    pub name: String,
    /// URL or path of the repository to connect.
    pub repository_url: String,
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
    /// Account requesting the disconnect.
    pub account_id: AccountId,
}

/// Successful project-disconnect response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DisconnectProjectResponse {
    /// The project that was disconnected.
    pub project_id: ProjectId,
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
    /// Repository URL or path the project is connected to.
    pub repository_url: String,
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
        }
    }
}
