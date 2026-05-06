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

use tanren_contract::{ActiveProjectView, ProjectView};
use tanren_testkit::{
    AccountHarness, ActorState, ApiHarness, CliHarness, FixtureSeed, HarnessKind, HarnessOutcome,
    InProcessHarness, McpHarness, TuiHarness, WebHarness,
};

/// Cucumber `World` shared across all Tanren BDD scenarios.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    /// Deterministic fixture seed.
    pub seed: FixtureSeed,
    /// Lazily initialized account-flow context.
    pub account: Option<AccountContext>,
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

    /// Refresh the account context with the harness chosen for the
    /// supplied scenario tags. Cucumber-rs does not give step bodies
    /// access to the active scenario's tags, so the BDD bin invokes
    /// this from a `Before` hook.
    pub async fn install_harness_for_tags<I, S>(&mut self, tags: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let kind = HarnessKind::from_tags(tags);
        let ctx = AccountContext::new_for(kind).await;
        self.account = Some(ctx);
    }
}

/// Per-scenario state carried by the cucumber world. Tracks per-actor
/// outcomes plus the active wire harness — all transport-specific
/// state lives inside the harness implementation.
pub struct AccountContext {
    pub harness: Box<dyn AccountHarness>,
    pub actors: HashMap<String, ActorState>,
    pub last_outcome: Option<HarnessOutcome>,
    pub invitations: HashSet<String>,
    pub last_project: Option<ProjectView>,
    pub last_project_account_id: Option<tanren_identity_policy::AccountId>,
    pub last_active_project: Option<Option<ActiveProjectView>>,
    pub captured_repository_identity: Option<String>,
    pub projects: Vec<ProjectView>,
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
            .field("has_last_project", &self.last_project.is_some())
            .field("last_project_account_id", &self.last_project_account_id)
            .field("last_active_project", &self.last_active_project)
            .field(
                "captured_repository_identity",
                &self.captured_repository_identity,
            )
            .field("project_count", &self.projects.len())
            .finish_non_exhaustive()
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
            HarnessKind::Web => Box::new(WebHarness::spawn().await.expect("WebHarness::spawn")),
        };
        Self {
            harness,
            actors: HashMap::new(),
            last_outcome: None,
            invitations: HashSet::new(),
            last_project: None,
            last_project_account_id: None,
            last_active_project: None,
            captured_repository_identity: None,
            projects: Vec::new(),
        }
    }
}

fn short_outcome_label(outcome: &HarnessOutcome) -> &'static str {
    match outcome {
        HarnessOutcome::SignedUp(_) => "SignedUp",
        HarnessOutcome::SignedIn(_) => "SignedIn",
        HarnessOutcome::AcceptedInvitation(_) => "AcceptedInvitation",
        HarnessOutcome::ProjectConnected(_) => "ProjectConnected",
        HarnessOutcome::ProjectCreated(_) => "ProjectCreated",
        HarnessOutcome::ActiveProject(_) => "ActiveProject",
        HarnessOutcome::Failure(_) => "Failure",
        HarnessOutcome::ProjectFailure(_) => "ProjectFailure",
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
    use tanren_identity_policy::AccountId;
    use tanren_provider_integrations::{
        HostId, ProviderAction, ProviderConnectionContext, SourceControlProvider,
    };
    use tanren_testkit::FixtureSeed;
    use tanren_testkit::FixtureSourceControlProvider;

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
        };
        assert_eq!(world.seed.value(), 42);
    }

    fn make_context(
        actor: &AccountId,
        host: &str,
        action: ProviderAction,
    ) -> ProviderConnectionContext {
        ProviderConnectionContext {
            actor: *actor,
            host: HostId::new(host.to_owned()),
            action,
        }
    }

    #[tokio::test]
    async fn fixture_create_repository_records_url_and_returns_info() {
        let actor = AccountId::fresh();
        let fixture =
            FixtureSourceControlProvider::new().with_actor_connection(actor, "gitlab.com");

        let ctx = make_context(
            &actor,
            "gitlab.com",
            ProviderAction::CreateRepository {
                name: "acme-widget".to_owned(),
            },
        );

        let info = fixture
            .create_repository(&ctx)
            .await
            .expect("create_repository should succeed for connected actor");

        assert_eq!(info.url, "https://gitlab.com/acme-widget");

        let created = fixture.created_repositories();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].url, "https://gitlab.com/acme-widget");
    }

    #[tokio::test]
    async fn fixture_create_repository_rejects_unconnected_actor() {
        let actor = AccountId::fresh();
        let fixture =
            FixtureSourceControlProvider::new().with_actor_connection(actor, "github.com");

        let other_actor = AccountId::fresh();
        let ctx = make_context(
            &other_actor,
            "gitlab.com",
            ProviderAction::CreateRepository {
                name: "oops".to_owned(),
            },
        );

        let err = fixture
            .create_repository(&ctx)
            .await
            .expect_err("should reject unconnected actor/host");

        assert!(
            matches!(err, tanren_provider_integrations::ProviderError::HostAccess(ref h) if h.as_str() == "gitlab.com"),
            "expected HostAccess error, got {err:?}"
        );
        assert!(fixture.created_repositories().is_empty());
    }

    #[tokio::test]
    async fn fixture_check_repo_access_succeeds_for_connected_actor() {
        let actor = AccountId::fresh();
        let repo_url = "https://github.com/acme/existing-repo";
        let fixture = FixtureSourceControlProvider::new()
            .with_actor_connection(actor, "github.com")
            .with_existing_repository(repo_url);

        let ctx = make_context(
            &actor,
            "github.com",
            ProviderAction::CheckRepoAccess {
                url: repo_url.to_owned(),
            },
        );

        let info = fixture
            .check_repo_access(&ctx)
            .await
            .expect("check_repo_access should succeed for connected actor with existing repo");

        assert_eq!(info.url, repo_url);
    }

    #[tokio::test]
    async fn fixture_check_repo_access_rejects_unknown_host() {
        let actor = AccountId::fresh();
        let fixture =
            FixtureSourceControlProvider::new().with_actor_connection(actor, "github.com");

        let ctx = make_context(
            &actor,
            "gitlab.com",
            ProviderAction::CheckRepoAccess {
                url: "https://gitlab.com/acme/repo".to_owned(),
            },
        );

        let err = fixture
            .check_repo_access(&ctx)
            .await
            .expect_err("should reject host with no actor connection");

        assert!(
            matches!(err, tanren_provider_integrations::ProviderError::HostAccess(ref h) if h.as_str() == "gitlab.com"),
            "expected HostAccess error, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fixture_not_configured_provider_rejects_all_calls() {
        let fixture = FixtureSourceControlProvider::new().with_not_configured();
        let actor = AccountId::fresh();
        let ctx = make_context(
            &actor,
            "github.com",
            ProviderAction::CreateRepository {
                name: "anything".to_owned(),
            },
        );

        let err = fixture
            .create_repository(&ctx)
            .await
            .expect_err("not-configured provider must reject");

        assert!(
            matches!(
                err,
                tanren_provider_integrations::ProviderError::NotConfigured
            ),
            "expected NotConfigured, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fixture_globally_accessible_host_allows_any_actor() {
        let fixture = FixtureSourceControlProvider::new()
            .with_accessible_host("github.com")
            .with_existing_repository("https://github.com/acme/pub-repo");

        let unrelated_actor = AccountId::fresh();
        let ctx = make_context(
            &unrelated_actor,
            "github.com",
            ProviderAction::CheckRepoAccess {
                url: "https://github.com/acme/pub-repo".to_owned(),
            },
        );

        let info = fixture
            .check_repo_access(&ctx)
            .await
            .expect("globally accessible host should allow any actor");

        assert_eq!(info.url, "https://github.com/acme/pub-repo");
    }
}
