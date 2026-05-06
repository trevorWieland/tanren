//! Project command/response wire shapes.
//!
//! Types for the project list view, active-project selector, and
//! project-scoped views. The spec-level attention indicators are
//! fixtured here — M-0007 owns real spec lifecycle semantics.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use utoipa::ToSchema;

/// External-facing view of a Tanren project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectView {
    /// Stable project id.
    pub id: ProjectId,
    /// Human-readable project name.
    pub name: String,
    /// Aggregated state summary for the project.
    pub state: ProjectStateSummary,
    /// Whether any spec in this project currently needs attention.
    pub needs_attention: bool,
    /// Specs within this project that currently need attention.
    pub attention_specs: Vec<AttentionSpecView>,
    /// Wall-clock time the project was created.
    pub created_at: DateTime<Utc>,
}

/// Aggregated state summary for a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStateSummary {
    /// Project is active and being worked on.
    Active,
    /// Project is paused.
    Paused,
    /// Project has been completed.
    Completed,
    /// Project has been archived.
    Archived,
}

/// A spec within a project that currently needs attention.
///
/// M-0007 owns real spec lifecycle semantics; at M-0003 time the
/// attention indicator is a fixtured boolean and reason string so the
/// project list view can render aggregated attention state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AttentionSpecView {
    /// Stable spec id.
    pub id: SpecId,
    /// Human-readable spec name.
    pub name: String,
    /// Why this spec needs attention (fixtured at M-0003).
    pub reason: String,
}

/// Views scoped to the currently active project. Switching projects
/// swaps these views while preserving the prior project's state for
/// resume.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectScopedViews {
    /// The project these views are scoped to.
    pub project_id: ProjectId,
    /// Specs belonging to the active project.
    pub specs: Vec<SpecId>,
    /// Loops belonging to the active project.
    pub loops: Vec<tanren_identity_policy::LoopId>,
    /// Milestones belonging to the active project.
    pub milestones: Vec<tanren_identity_policy::MilestoneId>,
}

/// Request to switch the active project for the caller's session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchProjectRequest {
    /// Account making the switch.
    pub account_id: AccountId,
    /// Project to activate.
    pub project_id: ProjectId,
}

/// Successful response after switching the active project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchProjectResponse {
    /// The newly active project.
    pub project: ProjectView,
    /// Scoped views for the newly active project.
    pub scoped: ProjectScopedViews,
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
    /// The caller does not have access to the requested project.
    UnauthorizedProjectAccess,
    /// The requested project does not exist.
    UnknownProject,
    /// The requested spec does not exist within the project.
    UnknownSpec,
}

impl ProjectFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnauthorizedProjectAccess => "unauthorized_project_access",
            Self::UnknownProject => "unknown_project",
            Self::UnknownSpec => "unknown_spec",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UnauthorizedProjectAccess => {
                "The caller does not have access to the requested project."
            }
            Self::UnknownProject => "The requested project does not exist.",
            Self::UnknownSpec => "The requested spec does not exist within the project.",
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::UnauthorizedProjectAccess => 403,
            Self::UnknownProject | Self::UnknownSpec => 404,
        }
    }
}
