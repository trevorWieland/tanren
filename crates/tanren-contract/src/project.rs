//! Project setup command/response wire shapes.
//!
//! These types are shared by api, mcp, cli, tui, and web when callers
//! connect an existing repository as a project or create a new project.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, DesignatedHost, ProjectId, RepositoryRef};
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
#[serde(deny_unknown_fields)]
pub struct ConnectProjectRepositoryCookieRequest {
    /// Canonical repository identity (`owner/name`).
    pub repository: RepositoryRef,
    /// Whether the newly connected project should be active immediately.
    pub select_as_active: bool,
}

impl ConnectProjectRepositoryCookieRequest {
    /// Build a full bearer-scoped request by injecting the session account.
    #[must_use]
    pub fn into_bearer_request(
        self,
        owning_account_id: AccountId,
    ) -> ConnectProjectRepositoryRequest {
        ConnectProjectRepositoryRequest {
            owning_account_id,
            repository: self.repository,
            select_as_active: self.select_as_active,
        }
    }
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
    pub designated_host: DesignatedHost,
    /// Whether the newly created project should be active immediately.
    pub select_as_active: bool,
}

/// Cookie-scoped API/web request for creating a project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectCookieRequest {
    /// Canonical repository identity (`owner/name`) to create.
    pub repository: RepositoryRef,
    /// Designated host where the repository should be created.
    pub designated_host: DesignatedHost,
    /// Whether the newly created project should be active immediately.
    pub select_as_active: bool,
}

impl CreateProjectCookieRequest {
    /// Build a full bearer-scoped request by injecting the session account.
    #[must_use]
    pub fn into_bearer_request(self, owning_account_id: AccountId) -> CreateProjectRequest {
        CreateProjectRequest {
            owning_account_id,
            repository: self.repository,
            designated_host: self.designated_host,
            select_as_active: self.select_as_active,
        }
    }
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
    /// Bounded pagination metadata for this page.
    pub pagination: ProjectPaginationView,
    /// Projection freshness metadata for this page.
    pub freshness: ProjectCollectionFreshnessView,
}

/// Query request for listing projects visible to an account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListVisibleProjectsRequest {
    /// Account whose visible projects should be listed.
    pub owning_account_id: AccountId,
    /// Pagination controls for this list request.
    #[serde(default)]
    pub page: ProjectPageRequest,
}

/// Cookie-scoped API/web request for listing projects visible to the
/// authenticated session account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ListVisibleProjectsCookieRequest {
    /// Pagination controls for this list request.
    #[serde(default)]
    pub page: ProjectPageRequest,
}

impl ListVisibleProjectsCookieRequest {
    /// Build a full bearer-scoped request by injecting the session account.
    #[must_use]
    pub fn into_bearer_request(self, owning_account_id: AccountId) -> ListVisibleProjectsRequest {
        ListVisibleProjectsRequest {
            owning_account_id,
            page: self.page,
        }
    }
}

/// Query request for reading active-project metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActiveProjectRequest {
    /// Account whose active-project metadata should be returned.
    pub owning_account_id: AccountId,
}

/// Cookie-scoped API/web request for active-project metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ActiveProjectCookieRequest {}

impl ActiveProjectCookieRequest {
    /// Build a full bearer-scoped request by injecting the session account.
    #[must_use]
    pub fn into_bearer_request(self, owning_account_id: AccountId) -> ActiveProjectRequest {
        ActiveProjectRequest { owning_account_id }
    }
}

/// Active-project projection for an account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActiveProjectView {
    /// Account that owns this active-project view.
    pub owning_account_id: AccountId,
    /// Active project, if one is currently selected.
    pub active_project: Option<ProjectView>,
}

/// Default project-list page size.
pub const PROJECT_LIST_DEFAULT_PAGE_SIZE: u16 = 25;
/// Maximum project-list page size.
pub const PROJECT_LIST_MAX_PAGE_SIZE: u16 = 100;

/// Pagination controls for project-list queries.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectPageRequest {
    /// Cursor pointing to the last project from the previous page.
    #[serde(default)]
    pub cursor: Option<ProjectListCursor>,
    /// Requested page size, bounded server-side to `[1, max_page_size]`.
    #[schemars(range(min = 1, max = 100))]
    #[schema(minimum = 1, maximum = 100)]
    #[serde(default = "default_project_list_page_size")]
    pub page_size: u16,
    /// Filter controls for this list request.
    #[serde(default)]
    pub filter: ProjectListFilterRequest,
    /// Sort controls for this list request.
    #[serde(default)]
    pub sort: ProjectListSortRequest,
}

impl Default for ProjectPageRequest {
    fn default() -> Self {
        Self {
            cursor: None,
            page_size: default_project_list_page_size(),
            filter: ProjectListFilterRequest::default(),
            sort: ProjectListSortRequest::default(),
        }
    }
}

/// Explicit filter fields supported by the project-list contract.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectListFilterRequest {
    /// Selection-state filter for visible projects.
    #[serde(default)]
    pub selection: ProjectListSelectionFilter,
}

/// Supported project-list selection filters.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema, Default,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectListSelectionFilter {
    /// Return all visible projects.
    #[default]
    All,
}

/// Explicit sort fields supported by the project-list contract.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectListSortRequest {
    /// Deterministic ordering for visible projects.
    #[serde(default)]
    pub order: ProjectListSortOrder,
}

/// Supported deterministic orderings for project-list pagination.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema, Default,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectListSortOrder {
    /// Sort by `active_selected_at DESC NULLS LAST, created_at DESC, id DESC`.
    #[default]
    ActiveSelectedThenCreatedDesc,
}

/// Stable cursor over the deterministic project-list ordering.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectListCursor {
    /// Active-selection timestamp used as the primary sort key.
    pub active_selected_at: Option<DateTime<Utc>>,
    /// Project creation timestamp used as a secondary sort key.
    pub created_at: DateTime<Utc>,
    /// Project id used as a final deterministic tie-breaker.
    pub project_id: ProjectId,
}

/// Pagination metadata for a project-list response page.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectPaginationView {
    /// The page size that was actually applied after bounding.
    pub page_size: u16,
    /// Default server page size when callers omit one.
    pub default_page_size: u16,
    /// Maximum server page size.
    pub max_page_size: u16,
    /// Whether another page exists after this one.
    pub has_more: bool,
    /// Cursor callers should send to request the next page.
    pub next_cursor: Option<ProjectListCursor>,
}

/// Projection freshness metadata for project lists.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectCollectionFreshnessView {
    /// The newest project-row timestamp visible in the account scope.
    pub as_of: Option<DateTime<Utc>>,
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
    /// Source-control host where this repository is bound.
    pub source_control_host: DesignatedHost,
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
    /// Another identical command is already in progress.
    InFlight,
    /// Command retries were throttled due to repeated failures.
    RateLimited,
    /// The actor does not have access to the requested account or repository.
    NoAccess,
    /// User-supplied input failed contract-level validation.
    ValidationFailed,
    /// Source-control provider is not configured for this environment.
    ProviderUnavailable,
    /// Source-control provider connectivity or operation failed.
    ProviderFailure,
}

/// Wire-visible project failure codes for `{code, summary}` error bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectFailureCode {
    AuthRequired,
    DuplicateRepository,
    InFlight,
    RateLimited,
    NoAccess,
    ValidationFailed,
    ProviderUnavailable,
    ProviderFailure,
    InternalError,
}

impl From<ProjectFailureReason> for ProjectFailureCode {
    fn from(reason: ProjectFailureReason) -> Self {
        match reason {
            ProjectFailureReason::AuthRequired => Self::AuthRequired,
            ProjectFailureReason::DuplicateRepository => Self::DuplicateRepository,
            ProjectFailureReason::InFlight => Self::InFlight,
            ProjectFailureReason::RateLimited => Self::RateLimited,
            ProjectFailureReason::NoAccess => Self::NoAccess,
            ProjectFailureReason::ValidationFailed => Self::ValidationFailed,
            ProjectFailureReason::ProviderUnavailable => Self::ProviderUnavailable,
            ProjectFailureReason::ProviderFailure => Self::ProviderFailure,
        }
    }
}

impl ProjectFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::DuplicateRepository => "duplicate_repository",
            Self::InFlight => "in_flight",
            Self::RateLimited => "rate_limited",
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
            Self::InFlight => {
                "A matching project command is already running. Retry once it completes."
            }
            Self::RateLimited => {
                "Project commands for this repository are temporarily rate-limited."
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
            Self::DuplicateRepository | Self::InFlight => 409,
            Self::RateLimited => 429,
            Self::NoAccess => 403,
            Self::ValidationFailed => 400,
            Self::ProviderUnavailable => 503,
            Self::ProviderFailure => 502,
        }
    }
}

const fn default_project_list_page_size() -> u16 {
    PROJECT_LIST_DEFAULT_PAGE_SIZE
}
