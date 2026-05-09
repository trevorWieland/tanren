//! Project setup command/response wire shapes.
//!
//! These types are shared by api, mcp, cli, tui, and web when callers
//! connect an existing repository as a project or create a new project.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, ProjectId, RepositoryRef};
use utoipa::ToSchema;

/// Connect an existing repository as a Tanren project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectRepositoryRequest {
    /// Account that owns the project.
    pub owning_account_id: AccountId,
    /// Canonical repository identity (`owner/name`).
    pub repository: RepositoryRef,
    /// Whether the newly connected project should be active immediately.
    pub select_as_active: bool,
}

/// Successful repository-connection response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectRepositoryResponse {
    /// Newly connected project.
    pub project: ProjectView,
}

/// Create a new project and repository in one action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateProjectRequest {
    /// Account that owns the project.
    pub owning_account_id: AccountId,
    /// Canonical repository identity (`owner/name`) to create.
    pub repository: RepositoryRef,
    /// Whether the newly created project should be active immediately.
    pub select_as_active: bool,
}

/// Successful project-creation response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateProjectResponse {
    /// Newly created project.
    pub project: ProjectView,
}

/// Wire projection for a project list/read response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectCollectionView {
    /// Account that owns this collection.
    pub owning_account_id: AccountId,
    /// Projects currently visible under the account.
    pub projects: Vec<ProjectView>,
}

/// External-facing project projection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectView {
    /// Stable project id.
    pub id: ProjectId,
    /// Account that owns the project.
    pub owning_account_id: AccountId,
    /// Repository connected to the project.
    pub repository: ProjectRepositoryView,
    /// Active-project selection metadata.
    pub selection: ProjectSelectionView,
    /// Current aggregate counts for neighboring planning subsystems.
    pub counts: ProjectCountsView,
    /// Wall-clock time the project was created.
    pub created_at: DateTime<Utc>,
}

/// Repository metadata bound to a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectRepositoryView {
    /// Canonical `owner/name` repository identity.
    pub repository: RepositoryRef,
}

/// Metadata about active-project selection state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectSelectionView {
    /// Whether this project is currently active for the owning account.
    pub is_active: bool,
    /// Wall-clock time this project became active, if active.
    pub selected_at: Option<DateTime<Utc>>,
}

/// Aggregated counts surfaced alongside project records.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectCountsView {
    /// Number of specs in this project.
    pub specs: u64,
    /// Number of milestones in this project.
    pub milestones: u64,
    /// Number of initiatives in this project.
    pub initiatives: u64,
}

/// Closed taxonomy of project-setup failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectFailureReason {
    /// A project already exists for this repository in the owning account.
    DuplicateRepository,
    /// The actor does not have access to the requested account or repository.
    NoAccess,
    /// User-supplied input failed contract-level validation.
    ValidationFailed,
    /// Source-control provider connectivity or operation failed.
    ProviderFailure,
}

impl ProjectFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateRepository => "duplicate_repository",
            Self::NoAccess => "no_access",
            Self::ValidationFailed => "validation_failed",
            Self::ProviderFailure => "provider_failure",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::DuplicateRepository => {
                "A project for the supplied repository already exists in this account."
            }
            Self::NoAccess => "The requested account, host, or repository is not accessible.",
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::ProviderFailure => {
                "The source-control provider could not complete the requested operation."
            }
        }
    }

    /// Recommended HTTP status for this failure on API/MCP surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::DuplicateRepository => 409,
            Self::NoAccess => 403,
            Self::ValidationFailed => 400,
            Self::ProviderFailure => 502,
        }
    }
}
