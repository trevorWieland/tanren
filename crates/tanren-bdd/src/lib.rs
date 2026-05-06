//! BDD step-definition home for Tanren.
//!
//! This is the only crate in the workspace permitted to define `#[test]`
//! items — `xtask check-rust-test-surface` mechanically rejects them
//! anywhere else. R-0001 sub-9 rewires the step bodies to dispatch
//! through the per-interface [`AccountHarness`] trait in
//! `tanren-testkit`, so the surface under proof matches the scenario's
//! interface tag — `@api` drives reqwest, `@cli` drives the binary,
//! `@mcp` drives the rmcp client, etc. `xtask check-bdd-wire-coverage`
//! mechanically rejects any step body that calls
//! `tanren_app_services::Handlers::*` directly.

pub mod steps;

use cucumber::World as CucumberWorld;
use std::collections::{HashMap, HashSet};

use tanren_testkit::{
    AccountHarness, ActorState, ApiHarness, CliHarness, FixtureSeed, HarnessKind, HarnessOutcome,
    InProcessHarness, McpHarness, ProjectApiHarness, ProjectCliHarness, ProjectHarness,
    ProjectInProcessHarness, ProjectMcpHarness, ProjectOutcome, ProjectTuiHarness,
    ProjectWebHarness, RepositoryFixture, TuiHarness, WebHarness,
};

use tanren_contract::{ProjectDependencyResponse, ProjectFailureReason, ProjectView};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, SpecId};

#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    pub seed: FixtureSeed,
    pub account: Option<AccountContext>,
    pub project: Option<ProjectContext>,
}

impl TanrenWorld {
    pub async fn ensure_account_ctx(&mut self) -> &mut AccountContext {
        if self.account.is_none() {
            self.account = Some(AccountContext::new_in_process().await);
        }
        self.account
            .as_mut()
            .expect("account context just initialized")
    }

    pub async fn ensure_project_ctx(&mut self) -> &mut ProjectContext {
        if self.project.is_none() {
            self.project = Some(ProjectContext::new_in_process().await);
        }
        self.project
            .as_mut()
            .expect("project context just initialized")
    }

    pub async fn install_harness_for_tags<I, S>(&mut self, tags: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let kind = HarnessKind::from_tags(tags);
        let account_ctx = AccountContext::new_for(kind).await;
        self.account = Some(account_ctx);
        let project_ctx = ProjectContext::new_for(kind).await;
        self.project = Some(project_ctx);
    }
}

pub struct AccountContext {
    pub harness: Box<dyn AccountHarness>,
    pub actors: HashMap<String, ActorState>,
    pub last_outcome: Option<HarnessOutcome>,
    pub invitations: HashSet<String>,
}

impl std::fmt::Debug for AccountContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountContext")
            .field("harness_kind", &self.harness.kind())
            .field("actors", &self.actors.keys().collect::<Vec<_>>())
            .field("invitations", &self.invitations)
            .field(
                "last_outcome",
                &self.last_outcome.as_ref().map(short_outcome_label),
            )
            .finish()
    }
}

impl AccountContext {
    pub async fn new_in_process() -> Self {
        Self::new_for(HarnessKind::InProcess).await
    }

    pub async fn new_for(kind: HarnessKind) -> Self {
        let harness: Box<dyn AccountHarness> = match kind {
            HarnessKind::InProcess => Box::new(
                InProcessHarness::new(kind)
                    .await
                    .expect("ephemeral SQLite must connect for BDD"),
            ),
            HarnessKind::Api => Box::new(ApiHarness::spawn().await.expect("ApiHarness::spawn")),
            HarnessKind::Cli => Box::new(CliHarness::spawn().await.expect("CliHarness::spawn")),
            HarnessKind::Mcp => Box::new(McpHarness::spawn().await.expect("McpHarness::spawn")),
            HarnessKind::Tui => Box::new(TuiHarness::spawn().await.expect("TuiHarness::spawn")),
            HarnessKind::Web => Box::new(WebHarness::spawn().await.expect("WebHarness::spawn")),
        };
        Self {
            harness,
            actors: HashMap::new(),
            last_outcome: None,
            invitations: HashSet::new(),
        }
    }
}

fn short_outcome_label(outcome: &HarnessOutcome) -> &'static str {
    match outcome {
        HarnessOutcome::SignedUp(_) => "SignedUp",
        HarnessOutcome::SignedIn(_) => "SignedIn",
        HarnessOutcome::AcceptedInvitation(_) => "AcceptedInvitation",
        HarnessOutcome::Failure(_) => "Failure",
        HarnessOutcome::Other(_) => "Other",
    }
}

pub struct ProjectContext {
    pub harness: Box<dyn ProjectHarness>,
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub last_outcome: Option<ProjectOutcome>,
    pub last_failure: Option<ProjectFailureReason>,
    pub connected_project_id: Option<ProjectId>,
    pub connected_project: Option<ProjectView>,
    pub temp_repo: Option<RepositoryFixture>,
    pub checksum_before: Option<String>,
    pub seeded_spec_ids: Vec<SpecId>,
    pub last_disconnect_unresolved: Vec<ProjectDependencyResponse>,
    pub last_listed_projects: Vec<ProjectView>,
    pub spec_count_before_disconnect: Option<usize>,
}

impl std::fmt::Debug for ProjectContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectContext")
            .field("harness_kind", &self.harness.kind())
            .field("account_id", &self.account_id)
            .field("org_id", &self.org_id)
            .field("connected_project", &self.connected_project)
            .field("seeded_spec_ids", &self.seeded_spec_ids)
            .field(
                "last_outcome",
                &self.last_outcome.as_ref().map(short_project_outcome_label),
            )
            .finish_non_exhaustive()
    }
}

impl ProjectContext {
    pub async fn new_in_process() -> Self {
        Self::new_for(HarnessKind::InProcess).await
    }

    pub async fn new_for(kind: HarnessKind) -> Self {
        let mut harness: Box<dyn ProjectHarness> = match kind {
            HarnessKind::InProcess => Box::new(
                ProjectInProcessHarness::new(kind)
                    .await
                    .expect("ephemeral SQLite must connect for BDD project harness"),
            ),
            HarnessKind::Api => Box::new(
                ProjectApiHarness::spawn()
                    .await
                    .expect("ProjectApiHarness::spawn"),
            ),
            HarnessKind::Cli => Box::new(
                ProjectCliHarness::spawn()
                    .await
                    .expect("ProjectCliHarness::spawn"),
            ),
            HarnessKind::Mcp => Box::new(
                ProjectMcpHarness::spawn()
                    .await
                    .expect("ProjectMcpHarness::spawn"),
            ),
            HarnessKind::Tui => Box::new(
                ProjectTuiHarness::spawn()
                    .await
                    .expect("ProjectTuiHarness::spawn"),
            ),
            HarnessKind::Web => Box::new(
                ProjectWebHarness::spawn()
                    .await
                    .expect("ProjectWebHarness::spawn"),
            ),
        };
        let (account_id, org_id) = harness
            .seed_account()
            .await
            .expect("seed project test account");
        Self {
            harness,
            account_id,
            org_id,
            last_outcome: None,
            last_failure: None,
            connected_project_id: None,
            connected_project: None,
            temp_repo: None,
            checksum_before: None,
            seeded_spec_ids: Vec::new(),
            last_disconnect_unresolved: Vec::new(),
            last_listed_projects: Vec::new(),
            spec_count_before_disconnect: None,
        }
    }
}

fn short_project_outcome_label(outcome: &ProjectOutcome) -> &'static str {
    match outcome {
        ProjectOutcome::Connected(_) => "Connected",
        ProjectOutcome::Disconnected(_) => "Disconnected",
        ProjectOutcome::Listed(_) => "Listed",
        ProjectOutcome::Reconnected(_) => "Reconnected",
        ProjectOutcome::Failure(_) => "Failure",
        ProjectOutcome::Other(_) => "Other",
    }
}

pub async fn run_features(features_dir: impl Into<std::path::PathBuf>) {
    TanrenWorld::cucumber()
        .before(|_feature, _rule, scenario, world| {
            let tags = scenario.tags.clone();
            Box::pin(async move {
                world.install_harness_for_tags(tags).await;
            })
        })
        .fail_on_skipped()
        .run_and_exit(features_dir.into())
        .await;
}

#[cfg(test)]
mod tests {
    use super::TanrenWorld;
    use tanren_testkit::FixtureSeed;

    #[test]
    fn world_default_is_constructible() {
        let world = TanrenWorld::default();
        assert_eq!(world.seed, FixtureSeed::default());
    }

    #[test]
    fn world_seed_round_trips() {
        let world = TanrenWorld {
            seed: FixtureSeed::new(42),
            account: None,
            project: None,
        };
        assert_eq!(world.seed.value(), 42);
    }
}
