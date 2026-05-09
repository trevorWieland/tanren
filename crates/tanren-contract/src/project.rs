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

/// Cookie-scoped API/web request for connecting an existing repository.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectRepositoryCookieRequest {
    /// Legacy compatibility shim for pre-session-scoped callers. Not part
    /// of the `OpenAPI` request schema.
    #[serde(default)]
    #[serde(rename = "owning_account_id")]
    #[schema(ignore)]
    pub legacy_owning_account_id: Option<AccountId>,
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
    /// Designated host where the repository should be created.
    pub designated_host: String,
    /// Whether the newly created project should be active immediately.
    pub select_as_active: bool,
}

/// Cookie-scoped API/web request for creating a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateProjectCookieRequest {
    /// Legacy compatibility shim for pre-session-scoped callers. Not part
    /// of the `OpenAPI` request schema.
    #[serde(default)]
    #[serde(rename = "owning_account_id")]
    #[schema(ignore)]
    pub legacy_owning_account_id: Option<AccountId>,
    /// Canonical repository identity (`owner/name`) to create.
    pub repository: RepositoryRef,
    /// Designated host where the repository should be created.
    pub designated_host: String,
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

/// Query request for listing projects visible to an account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListVisibleProjectsRequest {
    /// Account whose visible projects should be listed.
    pub owning_account_id: AccountId,
}

/// Cookie-scoped API/web request for listing projects visible to the
/// authenticated session account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, Default)]
pub struct ListVisibleProjectsCookieRequest {}

/// Query request for reading active-project metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActiveProjectRequest {
    /// Account whose active-project metadata should be returned.
    pub owning_account_id: AccountId,
}

/// Cookie-scoped API/web request for active-project metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, Default)]
pub struct ActiveProjectCookieRequest {}

/// Active-project projection for an account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActiveProjectView {
    /// Account that owns this active-project view.
    pub owning_account_id: AccountId,
    /// Active project, if one is currently selected.
    pub active_project: Option<ProjectView>,
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
    /// The request requires authentication and no valid session was present.
    AuthRequired,
    /// A project already exists for this repository in the owning account.
    DuplicateRepository,
    /// The actor does not have access to the requested account or repository.
    NoAccess,
    /// User-supplied input failed contract-level validation.
    ValidationFailed,
    /// Source-control provider is not configured for this environment.
    ProviderUnavailable,
    /// Source-control provider connectivity or operation failed.
    ProviderFailure,
}

impl ProjectFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::DuplicateRepository => "duplicate_repository",
            Self::NoAccess => "no_access",
            Self::ValidationFailed => "validation_failed",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ProviderFailure => "provider_failure",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AuthRequired => "Authentication is required or the session is missing/expired.",
            Self::DuplicateRepository => {
                "A project for the supplied repository already exists in this account."
            }
            Self::NoAccess => "The requested account, host, or repository is not accessible.",
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::ProviderUnavailable => {
                "No source-control provider is configured for this environment."
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
            Self::AuthRequired => 401,
            Self::DuplicateRepository => 409,
            Self::NoAccess => 403,
            Self::ValidationFailed => 400,
            Self::ProviderUnavailable => 503,
            Self::ProviderFailure => 502,
        }
    }
}
