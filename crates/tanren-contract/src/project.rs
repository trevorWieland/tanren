//! Project command/response wire shapes.
//!
//! Request/response surface for B-0025 (connect existing repository) and
//! B-0026 (create new project + repository). One project maps to exactly
//! one repository; the content counts start at zero (no specs, milestones,
//! or initiatives are scaffolded).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, RepositoryId};
use utoipa::ToSchema;

/// Connect an existing repository the caller already controls (B-0025).
///
/// The caller supplies the repository URL on a designated SCM host; Tanren
/// registers the project and links it to that repository. The repository
/// is not created or modified — only registered as the backing store.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConnectProjectRequest {
    /// Human-readable name for the new project.
    pub name: String,
    /// Fully-qualified URL of the existing repository on a supported SCM
    /// host. The handler validates that the caller has access via the
    /// configured SCM provider connection (R-0016).
    pub repository_url: String,
    /// Owning organization — `None` registers the project under the
    /// caller's personal scope.
    pub org: Option<OrgId>,
}

/// Create a new project and its backing repository in one step (B-0026).
///
/// The handler delegates repository creation to the designated SCM provider
/// (M-0009) using the caller's existing provider connection (R-0016), then
/// registers the project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateProjectRequest {
    /// Human-readable name for the new project.
    pub name: String,
    /// SCM host identifier where the repository will be created. The
    /// caller must have an active provider connection for this host.
    pub provider_host: String,
    /// Owning organization — `None` creates the project under the
    /// caller's personal scope.
    pub org: Option<OrgId>,
}

/// External-facing view of a registered project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectView {
    /// Stable project id.
    pub id: ProjectId,
    /// Human-readable name.
    pub name: String,
    /// Single backing repository (one project = one repository).
    pub repository: RepositoryView,
    /// Owning account.
    pub owner: AccountId,
    /// Owning organization — `None` for personal projects.
    pub org: Option<OrgId>,
    /// Wall-clock time the project was created.
    pub created_at: DateTime<Utc>,
    /// Current content counts (starts empty — no specs/milestones/initiatives
    /// are scaffolded during project registration).
    pub content_counts: ProjectContentCounts,
}

/// Identifies the single repository backing a project on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RepositoryView {
    /// Stable repository id allocated by Tanren.
    pub id: RepositoryId,
    /// Fully-qualified URL of the repository on the SCM host.
    pub url: String,
}

/// Summary of content counts for a project. A freshly registered project
/// starts with all counts at zero — no specs, milestones, or initiatives
/// are scaffolded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectContentCounts {
    /// Number of specs in the project.
    pub specs: u32,
    /// Number of milestones in the project.
    pub milestones: u32,
    /// Number of initiatives in the project.
    pub initiatives: u32,
}

impl ProjectContentCounts {
    /// Counts for a brand-new project — all zeroes.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            specs: 0,
            milestones: 0,
            initiatives: 0,
        }
    }
}

/// External-facing view of the caller's currently active project.
///
/// Returned by the "get active project" and "set active project" endpoints
/// (R-0020). Carries the full project view plus metadata about when the
/// project was activated in the caller's session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ActiveProjectView {
    /// The active project.
    pub project: ProjectView,
    /// Wall-clock time the project was activated for the caller.
    pub activated_at: DateTime<Utc>,
}

/// Closed taxonomy of project-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects a `ProjectFailureReason`
/// into the same wire shape so callers can match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProjectFailureReason {
    /// The caller does not have access to the target repository or
    /// provider connection.
    AccessDenied,
    /// The repository is already registered to another project (one
    /// project = one repository).
    DuplicateRepository,
    /// User-supplied input failed validation before any provider or
    /// persistence operations could run.
    ValidationFailed,
    /// The upstream SCM provider rejected the request or was unreachable.
    ProviderFailure,
    /// No SCM provider is configured for the deployment.
    ProviderNotConfigured,
}

impl ProjectFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AccessDenied => "access_denied",
            Self::DuplicateRepository => "duplicate_repository",
            Self::ValidationFailed => "validation_failed",
            Self::ProviderFailure => "provider_failure",
            Self::ProviderNotConfigured => "provider_not_configured",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AccessDenied => "You do not have access to the requested repository or provider.",
            Self::DuplicateRepository => "The repository is already connected to another project.",
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::ProviderFailure => "The SCM provider rejected the request or was unreachable.",
            Self::ProviderNotConfigured => "SCM provider is not configured.",
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AccessDenied => 403,
            Self::DuplicateRepository => 409,
            Self::ValidationFailed => 400,
            Self::ProviderFailure => 502,
            Self::ProviderNotConfigured => 503,
        }
    }
}
