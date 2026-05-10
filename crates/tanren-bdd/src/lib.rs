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
use std::path::PathBuf;

use tanren_testkit::{
    AccountHarness, ActorState, ApiHarness, CliHarness, FixtureSeed, HarnessKind, HarnessOutcome,
    InProcessHarness, McpHarness, ProjectHarness, TuiHarness, WebHarness,
};

/// Cucumber `World` shared across all Tanren BDD scenarios.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    /// Deterministic fixture seed.
    pub seed: FixtureSeed,
    /// Lazily initialized account-flow context.
    pub account: Option<AccountContext>,
    /// Lazily initialized project-setup context.
    pub project: Option<ProjectContext>,
}

impl TanrenWorld {
    /// Construct (or return) the lazy account context.
    pub async fn ensure_account_ctx(&mut self) -> &mut AccountContext {
        if self.account.is_none() {
            self.account = Some(AccountContext::new_in_process().await);
        }
        self.account
            .as_mut()
            .expect("account context just initialized")
    }

    /// Refresh the per-feature contexts with the harness chosen for the
    /// supplied scenario tags. Cucumber-rs does not give step bodies
    /// access to the active scenario's tags, so the BDD bin invokes
    /// this from a `Before` hook.
    pub async fn install_harness_for_tags<I, S>(&mut self, tags: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let kind = HarnessKind::from_tags(tags);
        self.account = Some(AccountContext::new_for(kind).await);
        self.project = Some(ProjectContext::new_for(kind).await);
    }

    /// Construct (or return) the lazy project context.
    pub async fn ensure_project_ctx(&mut self) -> &mut ProjectContext {
        if self.project.is_none() {
            self.project = Some(ProjectContext::new_in_process().await);
        }
        self.project
            .as_mut()
            .expect("project context just initialized")
    }
}

/// Per-scenario state carried by the cucumber world. Tracks per-actor
/// outcomes plus the active wire harness — all transport-specific
/// state lives inside the harness implementation.
pub struct AccountContext {
    /// Active wire harness for the current scenario.
    pub harness: Box<dyn AccountHarness>,
    /// Registry of actors by display name.
    pub actors: HashMap<String, ActorState>,
    /// The most recent action's outcome.
    pub last_outcome: Option<HarnessOutcome>,
    /// Per-scenario invitation tokens recorded by `Given a pending
    /// invitation token "..."` style steps.
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
    /// Build a context with the in-process harness — used for
    /// untagged scenarios.
    pub async fn new_in_process() -> Self {
        Self::new_for(HarnessKind::InProcess).await
    }

    /// Build a context with the harness matching the supplied tag
    /// kind. Falls back to the in-process harness if the requested
    /// transport fails to come up (e.g. a missing CLI binary on a
    /// fresh checkout) — the failure is recorded in `last_outcome`
    /// so it surfaces during the first step rather than blocking
    /// scenario discovery.
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
            // PR 11 ships the real-browser proof on the Node side via
            // `playwright-bdd`; the Rust path keeps in-process fallback
            // for fast feedback. See `tanren_testkit::harness::web`.
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

/// Per-scenario state carried by the cucumber world for project-setup flows.
pub struct ProjectContext {
    /// Active wire harness for the current scenario.
    pub harness: Box<dyn ProjectHarness>,
    /// Per-actor state keyed by display name.
    pub actors: HashMap<String, ProjectActorState>,
    /// Fixture repository metadata keyed by canonical repository identity.
    pub repositories: HashMap<String, RepositoryFixtureState>,
    /// Fixture host access policy keyed by normalized host label.
    pub hosts: HashMap<String, HostFixtureState>,
    /// Most recent project failure code, if any.
    pub last_failure_code: Option<String>,
    /// Most recent project failure summary, if any.
    pub last_failure_summary: Option<String>,
}

impl std::fmt::Debug for ProjectContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectContext")
            .field("harness_kind", &self.harness.kind())
            .field("actors", &self.actors.keys().collect::<Vec<_>>())
            .field(
                "repositories",
                &self.repositories.keys().collect::<Vec<_>>(),
            )
            .field("hosts", &self.hosts.keys().collect::<Vec<_>>())
            .field("last_failure_code", &self.last_failure_code)
            .field("last_failure_summary", &self.last_failure_summary)
            .finish()
    }
}

impl ProjectContext {
    /// Build a project context with the in-process harness.
    pub async fn new_in_process() -> Self {
        Self::new_for(HarnessKind::InProcess).await
    }

    /// Build a project context with the harness matching the supplied tag kind.
    pub async fn new_for(kind: HarnessKind) -> Self {
        let harness: Box<dyn ProjectHarness> = match kind {
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
            repositories: HashMap::new(),
            hosts: HashMap::new(),
            last_failure_code: None,
            last_failure_summary: None,
        }
    }
}

/// Minimal per-actor state used by project steps.
#[derive(Debug, Default, Clone)]
pub struct ProjectActorState {
    /// The actor's account id used for project commands.
    pub account_id: Option<tanren_identity_policy::AccountId>,
    /// Last repository successfully connected by this actor.
    pub last_connected_repository: Option<tanren_identity_policy::RepositoryRef>,
    /// Last repository successfully created by this actor.
    pub last_created_repository: Option<tanren_identity_policy::RepositoryRef>,
    /// Last designated host this actor attempted for project creation.
    pub last_designated_host: Option<String>,
}

/// Fixture repository metadata tracked by project scenarios.
#[derive(Debug, Clone)]
pub struct RepositoryFixtureState {
    /// Canonical repository identity (`owner/name`).
    pub repository: tanren_identity_policy::RepositoryRef,
    /// Deterministic fingerprint used by the scenario.
    pub fingerprint: String,
    /// Number of pre-existing commits in the fixture repository.
    pub prior_commits: usize,
}

/// Fixture metadata for a designated source-control host.
#[derive(Debug, Clone)]
pub struct HostFixtureState {
    /// Normalized host label.
    pub host: String,
    /// Whether creation at this host should be considered accessible.
    pub can_create: bool,
    /// Repositories observed as created at this host in this scenario.
    pub created_repositories: HashSet<tanren_identity_policy::RepositoryRef>,
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

/// Run the cucumber harness against the supplied features directory.
/// The harness installs a `Before` hook that selects the per-interface
/// wire harness from the active scenario's tags.
pub async fn run_features(features_dir: impl Into<PathBuf>) {
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
    //! Unit-test guards for the BDD harness machinery itself.

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
