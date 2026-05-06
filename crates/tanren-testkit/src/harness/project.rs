//! Per-interface project-flow wire-harness trait (R-0020 S-07).
//!
//! Mirrors [`super::AccountHarness`]: every implementation drives the
//! matching real surface end-to-end. Step bodies in `tanren-bdd` dispatch
//! through [`ProjectHarness`] and never call `Handlers` directly.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tanren_contract::{AttentionSpecView, ProjectScopedViews, ProjectView, SwitchProjectResponse};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};

use super::{HarnessError, HarnessKind, HarnessResult};

/// Fixture for seeding a project into the harness's backing store.
#[derive(Debug, Clone)]
pub struct HarnessProjectFixture {
    /// Pre-allocated project id.
    pub id: ProjectId,
    /// Owning account.
    pub account_id: AccountId,
    /// Human-readable project name.
    pub name: String,
    /// State string (`"active"`, `"paused"`, `"completed"`, `"archived"`).
    pub state: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Fixture for seeding a spec with optional attention flag.
#[derive(Debug, Clone)]
pub struct HarnessSpecFixture {
    /// Pre-allocated spec id.
    pub id: SpecId,
    /// Parent project.
    pub project_id: ProjectId,
    /// Human-readable spec name.
    pub name: String,
    /// Whether this spec needs attention.
    pub needs_attention: bool,
    /// Why this spec needs attention (fixtured at M-0003).
    pub attention_reason: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Per-interface seam for project-flow BDD scenarios. Every
/// implementation drives the matching real surface: `@api` through
/// reqwest, `@cli` through subprocess, `@mcp` through the rmcp client,
/// etc. The trait keeps `Handlers` out of `tanren-bdd`.
#[async_trait]
pub trait ProjectHarness: Send + std::fmt::Debug {
    /// Identifier for diagnostic output.
    fn kind(&self) -> HarnessKind;

    /// Seed a project into the backing store. Returns the allocated id.
    async fn seed_project(&mut self, fixture: HarnessProjectFixture) -> HarnessResult<ProjectId>;

    /// Seed a spec into the backing store. Returns the allocated id.
    async fn seed_spec(&mut self, fixture: HarnessSpecFixture) -> HarnessResult<SpecId>;

    /// Seed per-project view state for a given account/project pair.
    async fn seed_view_state(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        state: Value,
    ) -> HarnessResult<()>;

    /// List every project visible to the account, with attention
    /// indicators aggregated from spec-level flags.
    async fn list_projects(&mut self, account_id: AccountId) -> HarnessResult<Vec<ProjectView>>;

    /// Switch the active project for the account.
    async fn switch_active_project(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse>;

    /// Drill down into a specific attention-flagged spec.
    async fn attention_spec(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        spec_id: SpecId,
    ) -> HarnessResult<AttentionSpecView>;

    /// Read scoped views (specs, loops, milestones) for the currently
    /// active project.
    async fn project_scoped_views(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ProjectScopedViews>;
}

/// Map a [`HarnessError`] into a failure-appropriate outcome.
/// Used by project step bodies to record failures in the World.
pub fn record_project_failure(err: HarnessError) -> ProjectOutcome {
    match err {
        HarnessError::Project(reason, _) => ProjectOutcome::Failure(reason),
        HarnessError::Transport(msg) => ProjectOutcome::Other(format!("transport: {msg}")),
        HarnessError::Account(reason, _) => {
            ProjectOutcome::Other(format!("account error in project flow: {reason:?}"))
        }
    }
}

/// Outcome of a project-flow action. Carried by the BDD World so
/// downstream `Then` steps can assert on results.
#[derive(Debug, Clone)]
pub enum ProjectOutcome {
    /// Project was seeded successfully.
    SeededProject(ProjectId),
    /// Spec was seeded successfully.
    SeededSpec(SpecId),
    /// View state was seeded.
    SeededViewState,
    /// Project list returned successfully.
    Listed(Vec<ProjectView>),
    /// Active project switched successfully.
    Switched(SwitchProjectResponse),
    /// Attention spec drill-down succeeded.
    DrilledSpec(AttentionSpecView),
    /// Scoped views read succeeded.
    ScopedViews(ProjectScopedViews),
    /// Project-flow taxonomy failure.
    Failure(tanren_contract::ProjectFailureReason),
    /// Non-taxonomy infrastructure failure.
    Other(String),
}
