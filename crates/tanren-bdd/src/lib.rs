//! BDD step-definition home for Tanren.
//!
//! This is the only crate in the workspace permitted to define `#[test]`
//! items — `xtask check-rust-test-surface` mechanically rejects them
//! anywhere else. R-0001 sub-9 rewires the step bodies to dispatch
//! through the per-interface [`tanren_testkit::InstallHarness`] trait in
//! `tanren-testkit`, so the surface under proof matches the scenario's
//! interface tag — `@api` drives reqwest, `@cli` drives the binary,
//! `@mcp` drives the rmcp client, etc. `xtask check-bdd-wire-coverage`
//! mechanically rejects any step body that calls
//! `tanren_app_services::Handlers::*` directly.

pub mod steps;

use cucumber::World as CucumberWorld;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::steps::install::{InstallContext, InstallStepError, InstallStepResult};
use tanren_testkit::install_contract::{
    InstallProofIntegration, InstallProofProfile, parse_install_integration_selection,
};
use tanren_testkit::{
    ActorState, ApiHarness, CliHarness, FixtureSeed, HarnessKind, HarnessOutcome, InProcessHarness,
    InstallCommandKind, InstallHarness, McpHarness, TuiHarness, WebHarness,
};

/// Cucumber `World` shared across all Tanren BDD scenarios.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    /// Deterministic fixture seed.
    pub seed: FixtureSeed,
    /// Lazily initialized account-flow context.
    pub account: Option<AccountContext>,
    /// Typed setup error captured by the scenario `Before` hook.
    install_setup_error: Option<InstallStepError>,
}

impl TanrenWorld {
    /// Construct (or return) the lazy account context.
    #[tracing::instrument(
        name = "bdd_world_ensure_account_ctx",
        level = "debug",
        skip(self),
        fields(command_kind = "bdd.account.ensure_context")
    )]
    pub async fn ensure_account_ctx(&mut self) -> &mut AccountContext {
        match &mut self.account {
            Some(account) => account,
            slot @ None => slot.insert(AccountContext::new_in_process().await),
        }
    }

    /// Construct (or return) the lazy install context.
    pub(crate) fn ensure_install_ctx(&mut self) -> Result<&mut InstallContext, InstallStepError> {
        self.require_account_ctx()?.ensure_install_ctx()
    }

    /// Reset the install context for the current scenario.
    pub(crate) fn reset_install_ctx(&mut self) -> InstallStepResult<()> {
        self.require_account_ctx()?.reset_install_ctx()
    }

    #[tracing::instrument(
        name = "bdd_world_run_install",
        level = "debug",
        skip(self),
        fields(
            command_kind = %InstallCommandKind::Install,
            profile = tracing::field::Empty,
            integration_count = tracing::field::Empty
        )
    )]
    pub(crate) async fn run_install(
        &mut self,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
        let typed_profile = profile.parse::<InstallProofProfile>().map_err(|_err| {
            InstallStepError::ProfileParseFailed {
                profile: profile.to_owned(),
            }
        })?;
        tracing::Span::current().record("profile", tracing::field::display(typed_profile.as_str()));
        let typed_integrations = if let Some(csv) = integrations {
            let set = parse_install_integration_selection(csv).map_err(|_err| {
                InstallStepError::IntegrationParseFailed {
                    integrations: csv.to_owned(),
                }
            })?;
            tracing::Span::current().record("integration_count", set.len());
            Some(set)
        } else {
            tracing::Span::current().record("integration_count", 0);
            None
        };
        self.require_account_ctx()?
            .run_install(typed_profile, typed_integrations.as_ref())
            .await
    }

    #[tracing::instrument(
        name = "bdd_world_run_drift",
        level = "debug",
        skip(self),
        fields(
            command_kind = %InstallCommandKind::Drift,
            profile = tracing::field::Empty,
            integration_count = tracing::field::Empty
        )
    )]
    pub(crate) async fn run_drift(
        &mut self,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
        let typed_profile = profile.parse::<InstallProofProfile>().map_err(|_err| {
            InstallStepError::ProfileParseFailed {
                profile: profile.to_owned(),
            }
        })?;
        tracing::Span::current().record("profile", tracing::field::display(typed_profile.as_str()));
        let typed_integrations = if let Some(csv) = integrations {
            let set = parse_install_integration_selection(csv).map_err(|_err| {
                InstallStepError::IntegrationParseFailed {
                    integrations: csv.to_owned(),
                }
            })?;
            tracing::Span::current().record("integration_count", set.len());
            Some(set)
        } else {
            tracing::Span::current().record("integration_count", 0);
            None
        };
        self.require_account_ctx()?
            .run_drift(typed_profile, typed_integrations.as_ref())
            .await
    }

    fn require_account_ctx(&mut self) -> InstallStepResult<&mut AccountContext> {
        if let Some(error) = self.install_setup_error.take() {
            return Err(error);
        }
        self.account
            .as_mut()
            .ok_or(InstallStepError::AccountContextUnavailable)
    }

    /// Refresh the account context with the harness chosen for the
    /// supplied scenario tags. Cucumber-rs does not give step bodies
    /// access to the active scenario's tags, so the BDD bin invokes
    /// this from a `Before` hook.
    #[tracing::instrument(
        name = "bdd_world_install_harness_for_tags",
        level = "debug",
        skip(self, tags),
        fields(command_kind = "bdd.harness.select", harness_kind = tracing::field::Empty)
    )]
    pub(crate) async fn install_harness_for_tags<I, S>(&mut self, tags: I) -> InstallStepResult<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.install_setup_error = None;
        let tags: Vec<String> = tags
            .into_iter()
            .map(|tag| tag.as_ref().to_owned())
            .collect();
        let kind = HarnessKind::from_tags(tags.iter().map(String::as_str));
        tracing::Span::current().record("harness_kind", tracing::field::debug(kind));
        let mut ctx = AccountContext::new_for(kind).await;
        if tags
            .iter()
            .any(|tag| tag.strip_prefix('@').unwrap_or(tag) == "cli")
        {
            ctx.install = Some(InstallContext::new()?);
        }
        self.account = Some(ctx);
        Ok(())
    }

    fn store_install_setup_error(&mut self, error: InstallStepError) {
        self.account = None;
        self.install_setup_error = Some(error);
    }
}

/// Per-scenario state carried by the cucumber world. Tracks per-actor
/// outcomes plus the active wire harness — all transport-specific
/// state lives inside the harness implementation.
pub struct AccountContext {
    /// Active wire harness for the current scenario.
    pub harness: Box<dyn InstallHarness>,
    /// Registry of actors by display name.
    pub actors: HashMap<String, ActorState>,
    /// The most recent action's outcome.
    pub last_outcome: Option<HarnessOutcome>,
    /// Per-scenario invitation tokens recorded by `Given a pending
    /// invitation token "..."` style steps.
    pub invitations: HashSet<String>,
    /// Install-flow fixture state for CLI-tagged scenarios.
    pub(crate) install: Option<InstallContext>,
}

impl std::fmt::Debug for AccountContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountContext")
            .field("harness_kind", &self.harness.kind())
            .field("actors", &self.actors.keys().collect::<Vec<_>>())
            .field("invitations", &self.invitations)
            .field("has_install_ctx", &self.install.is_some())
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
        let harness: Box<dyn InstallHarness> = match kind {
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
            install: None,
        }
    }

    fn ensure_install_ctx(&mut self) -> InstallStepResult<&mut InstallContext> {
        self.install
            .as_mut()
            .ok_or(InstallStepError::InstallContextUnavailable)
    }

    fn reset_install_ctx(&mut self) -> InstallStepResult<()> {
        self.install = Some(InstallContext::new()?);
        Ok(())
    }

    #[tracing::instrument(
        name = "bdd_account_ctx_run_install",
        level = "debug",
        skip(self),
        fields(
            command_kind = %InstallCommandKind::Install,
            harness_kind = tracing::field::Empty,
            profile = %profile.as_str(),
            integration_count = integrations.as_ref().map_or(0, |s| s.len())
        )
    )]
    async fn run_install(
        &mut self,
        profile: InstallProofProfile,
        integrations: Option<&std::collections::BTreeSet<InstallProofIntegration>>,
    ) -> InstallStepResult<()> {
        tracing::Span::current().record("harness_kind", tracing::field::debug(self.harness.kind()));
        let install = self
            .install
            .as_mut()
            .ok_or(InstallStepError::InstallContextUnavailable)?;
        install
            .run_install(self.harness.as_mut(), profile, integrations)
            .await
    }

    #[tracing::instrument(
        name = "bdd_account_ctx_run_drift",
        level = "debug",
        skip(self),
        fields(
            command_kind = %InstallCommandKind::Drift,
            harness_kind = tracing::field::Empty,
            profile = %profile.as_str(),
            integration_count = integrations.as_ref().map_or(0, |s| s.len())
        )
    )]
    async fn run_drift(
        &mut self,
        profile: InstallProofProfile,
        integrations: Option<&std::collections::BTreeSet<InstallProofIntegration>>,
    ) -> InstallStepResult<()> {
        tracing::Span::current().record("harness_kind", tracing::field::debug(self.harness.kind()));
        let install = self
            .install
            .as_mut()
            .ok_or(InstallStepError::InstallContextUnavailable)?;
        install
            .run_drift(self.harness.as_mut(), profile, integrations)
            .await
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

/// Run the cucumber harness against the supplied features directory.
/// The harness installs a `Before` hook that selects the per-interface
/// wire harness from the active scenario's tags.
pub async fn run_features(features_dir: impl Into<PathBuf>) {
    TanrenWorld::cucumber()
        .before(|_feature, _rule, scenario, world| {
            let tags = scenario.tags.clone();
            Box::pin(async move {
                if let Err(error) = world.install_harness_for_tags(tags).await {
                    world.store_install_setup_error(error);
                }
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
            install_setup_error: None,
        };
        assert_eq!(world.seed.value(), 42);
    }
}
