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
use tanren_testkit::install_contract::{InstallProofProfile, parse_install_integration_selection};
use tanren_testkit::{
    ActorState, ApiHarness, CliHarness, FixtureSeed, HarnessKind, HarnessOutcome, InProcessHarness,
    InstallCommandKind, InstallHarness, McpHarness, TuiHarness, WebHarness,
};
/// Explicit world setup state machine.
///
/// Replaces the previous `Option<AccountContext>` +
/// `Option<InstallStepError>` pair. Each variant is a distinct, named
/// state the world can be in — no `take()` consumption needed to
/// inspect setup errors.
#[derive(Debug, Default)]
enum WorldSetupState {
    /// World has not been initialized by a `Before` hook.
    #[default]
    NotInitialized,
    /// Account harness is ready; install context is separate.
    Ready {
        account: Box<AccountContext>,
        install: Option<InstallContext>,
    },
    /// Setup failed during the `Before` hook; the typed error is
    /// preserved for step bodies to inspect without consuming it.
    SetupFailed(InstallStepError),
}
/// Determine whether scenario tags indicate an install-command fixture
/// is needed. Install fixtures are selected from behavior/interface
/// needs: any scenario tagged with a behavior that requires install
/// command fixture support (B-0068, B-0069, B-0070) needs the fixture,
/// regardless of which interface tag carries the scenario.
fn install_context_needed_from_tags<I, S>(tags: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut found_install_behavior = false;
    let mut found_interface = false;
    for tag in tags {
        let raw = tag.as_ref();
        let normalized = raw.strip_prefix('@').unwrap_or(raw);
        match normalized {
            "B-0068" | "B-0069" | "B-0070" => found_install_behavior = true,
            "api" | "cli" | "mcp" | "tui" | "web" => found_interface = true,
            _ => {}
        }
    }
    found_install_behavior && found_interface
}
/// Cucumber `World` shared across all Tanren BDD scenarios.
///
/// Install setup state is carried explicitly and separately from
/// account-flow state. The [`WorldSetupState`] enum makes the current
/// lifecycle phase machine-readable and prevents the need to consume
/// setup errors with `take()`.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    /// Deterministic fixture seed.
    pub seed: FixtureSeed,
    setup_state: WorldSetupState,
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
        let state = std::mem::take(&mut self.setup_state);
        let (account, install) = match state {
            WorldSetupState::NotInitialized => (AccountContext::new_in_process().await, None),
            WorldSetupState::Ready { account, install } => (*account, install),
            WorldSetupState::SetupFailed(error) => {
                let account = AccountContext::new_in_process().await;
                tracing::warn!(
                    error = %error,
                    "scenario before-hook failed; falling back to in-process account context"
                );
                (account, None)
            }
        };
        self.setup_state = WorldSetupState::Ready {
            account: Box::new(account),
            install,
        };
        match &mut self.setup_state {
            WorldSetupState::Ready { account, .. } => account,
            _ => unreachable!("just set Ready"),
        }
    }
    /// Construct (or return) the lazy install context.
    pub(crate) fn ensure_install_ctx(&mut self) -> Result<&mut InstallContext, InstallStepError> {
        self.propagate_setup_failure()?;
        if matches!(
            &self.setup_state,
            WorldSetupState::Ready { install: None, .. }
        ) {
            let ctx = InstallContext::new()?;
            self.set_install_ctx(ctx);
        }
        match &mut self.setup_state {
            WorldSetupState::Ready {
                install: Some(ctx), ..
            } => Ok(ctx),
            WorldSetupState::Ready { install: None, .. } => {
                Err(InstallStepError::InstallContextUnavailable)
            }
            _ => unreachable!("propagate_setup_failure handles these"),
        }
    }
    /// Reset the install context for the current scenario.
    pub(crate) fn reset_install_ctx(&mut self) -> InstallStepResult<()> {
        self.propagate_setup_failure()?;
        self.require_account_ctx_mut()?;
        let ctx = InstallContext::new()?;
        self.set_install_ctx(ctx);
        Ok(())
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
        let typed_profile = profile.parse::<InstallProofProfile>();
        let typed_integrations = integrations
            .map(parse_install_integration_selection)
            .transpose();

        let raw_profile = if typed_profile.is_err() {
            Some(profile.to_owned())
        } else {
            None
        };
        let raw_integrations = if typed_integrations.as_ref().is_err() {
            integrations.map(ToOwned::to_owned)
        } else {
            None
        };

        let valid_profile = typed_profile.unwrap_or(InstallProofProfile::RustCargo);
        let valid_integrations = typed_integrations.unwrap_or(None);

        tracing::Span::current().record("profile", tracing::field::display(valid_profile.as_str()));
        tracing::Span::current().record(
            "integration_count",
            valid_integrations
                .as_ref()
                .map_or(0, std::collections::BTreeSet::len),
        );

        match &mut self.setup_state {
            WorldSetupState::Ready {
                account,
                install: Some(install),
            } => {
                install
                    .run_install(
                        account.harness.as_mut(),
                        valid_profile,
                        valid_integrations.as_ref(),
                        raw_profile,
                        raw_integrations,
                    )
                    .await
            }
            WorldSetupState::Ready { install: None, .. } => {
                Err(InstallStepError::InstallContextUnavailable)
            }
            WorldSetupState::NotInitialized => Err(InstallStepError::AccountContextUnavailable),
            WorldSetupState::SetupFailed(_) => {
                unreachable!("propagate_setup_failure must be called first")
            }
        }
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
        let typed_profile = profile.parse::<InstallProofProfile>();
        let typed_integrations = integrations
            .map(parse_install_integration_selection)
            .transpose();

        let raw_profile = if typed_profile.is_err() {
            Some(profile.to_owned())
        } else {
            None
        };
        let raw_integrations = if typed_integrations.as_ref().is_err() {
            integrations.map(ToOwned::to_owned)
        } else {
            None
        };

        let valid_profile = typed_profile.unwrap_or(InstallProofProfile::RustCargo);
        let valid_integrations = typed_integrations.unwrap_or(None);

        tracing::Span::current().record("profile", tracing::field::display(valid_profile.as_str()));
        tracing::Span::current().record(
            "integration_count",
            valid_integrations
                .as_ref()
                .map_or(0, std::collections::BTreeSet::len),
        );

        match &mut self.setup_state {
            WorldSetupState::Ready {
                account,
                install: Some(install),
            } => {
                install
                    .run_drift(
                        account.harness.as_mut(),
                        valid_profile,
                        valid_integrations.as_ref(),
                        raw_profile,
                        raw_integrations,
                    )
                    .await
            }
            WorldSetupState::Ready { install: None, .. } => {
                Err(InstallStepError::InstallContextUnavailable)
            }
            WorldSetupState::NotInitialized => Err(InstallStepError::AccountContextUnavailable),
            WorldSetupState::SetupFailed(_) => {
                unreachable!("propagate_setup_failure must be called first")
            }
        }
    }
    /// Refresh the account context with the harness chosen for the
    /// supplied scenario tags. Cucumber-rs does not give step bodies
    /// access to the active scenario's tags, so the BDD bin invokes
    /// this from a `Before` hook.
    ///
    /// Install fixture setup is selected from behavior/interface needs
    /// rather than hard-coded tag checks — any scenario tagged with an
    /// install-related behavior (B-0068, B-0069, B-0070) and an interface
    /// tag gets the shared install-command fixture context.
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
        let tags: Vec<String> = tags
            .into_iter()
            .map(|tag| tag.as_ref().to_owned())
            .collect();
        let kind = HarnessKind::from_tags(tags.iter().map(String::as_str));
        tracing::Span::current().record("harness_kind", tracing::field::debug(kind));

        let needs_install = install_context_needed_from_tags(tags.iter().map(String::as_str));
        let install = if needs_install {
            Some(InstallContext::new()?)
        } else {
            None
        };

        let account = AccountContext::new_for(kind).await;
        self.setup_state = WorldSetupState::Ready {
            account: Box::new(account),
            install,
        };
        Ok(())
    }
    /// Store a setup failure so that subsequent step bodies observe the
    /// typed error. Unlike the previous `take()` approach, the error is
    /// preserved in the [`WorldSetupState::SetupFailed`] variant and
    /// read via [`propagate_setup_failure`] without consumption.
    fn store_install_setup_error(&mut self, error: InstallStepError) {
        self.setup_state = WorldSetupState::SetupFailed(error);
    }
    /// If the world is in [`WorldSetupState::SetupFailed`], extract the
    /// error and transition the world to `NotInitialized`. Unlike the
    /// previous `take()` on `Option<InstallStepError>`, this method
    /// replaces the entire setup state atomically and does not leave a
    /// partially-consumed `Option` behind.
    fn propagate_setup_failure(&mut self) -> InstallStepResult<()> {
        if matches!(&self.setup_state, WorldSetupState::SetupFailed(_)) {
            let state = std::mem::take(&mut self.setup_state);
            match state {
                WorldSetupState::SetupFailed(error) => {
                    return Err(InstallStepError::SetupFailed {
                        source: Box::new(error),
                    });
                }
                _ => unreachable!("guarded by matches! above"),
            }
        }
        Ok(())
    }
    /// Obtain a mutable reference to the account context, propagating
    /// setup failures or missing-context errors.
    fn require_account_ctx_mut(&mut self) -> InstallStepResult<&mut AccountContext> {
        self.propagate_setup_failure()?;
        match &mut self.setup_state {
            WorldSetupState::Ready { account, .. } => Ok(account),
            WorldSetupState::NotInitialized => Err(InstallStepError::AccountContextUnavailable),
            WorldSetupState::SetupFailed(_) => {
                unreachable!("propagate_setup_failure handled SetupFailed")
            }
        }
    }
    /// Set the install context slot within the Ready variant.
    fn set_install_ctx(&mut self, ctx: InstallContext) {
        if let WorldSetupState::Ready { install, .. } = &mut self.setup_state {
            *install = Some(ctx);
        }
    }
}

/// Per-scenario account-flow context.
///
/// Owns the per-interface wire harness and per-actor state for account-
/// flow scenarios (sign-up, sign-in, accept-invitation). Install fixture
/// state is carried separately on [`TanrenWorld`] to decouple install-
/// flow concerns from account-flow concerns.
#[derive(Debug)]
pub struct AccountContext {
    harness: Box<dyn InstallHarness>,
    actors: HashMap<String, ActorState>,
    last_outcome: Option<HarnessOutcome>,
    invitations: HashSet<String>,
}

impl AccountContext {
    /// Build an in-process account context as the default/fallback.
    async fn new_in_process() -> Self {
        let harness: Box<dyn InstallHarness> = Box::new(
            InProcessHarness::new(HarnessKind::InProcess)
                .await
                .expect("InProcessHarness::new"),
        );
        Self {
            harness,
            actors: HashMap::new(),
            last_outcome: None,
            invitations: HashSet::new(),
        }
    }

    /// Build an account context for the requested harness kind.
    async fn new_for(kind: HarnessKind) -> Self {
        let harness: Box<dyn InstallHarness> = match kind {
            HarnessKind::InProcess => Box::new(
                InProcessHarness::new(kind)
                    .await
                    .expect("InProcessHarness::new"),
            ),
            HarnessKind::Api => Box::new(ApiHarness::spawn().await.expect("ApiHarness::spawn")),
            HarnessKind::Cli => Box::new(CliHarness::spawn().await.expect("CliHarness::spawn")),
            HarnessKind::Mcp => Box::new(McpHarness::spawn().await.expect("McpHarness::spawn")),
            HarnessKind::Tui => Box::new(TuiHarness::spawn().await.expect("TuiHarness::spawn")),
            // PR 11: real-browser proof via playwright-bdd; Rust keeps in-process fallback.
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

    use super::{TanrenWorld, WorldSetupState, install_context_needed_from_tags};
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
            setup_state: WorldSetupState::NotInitialized,
        };
        assert_eq!(world.seed.value(), 42);
    }

    #[test]
    fn install_context_needed_for_b0068_with_cli() {
        let tags = &["@B-0068", "@positive", "@cli"];
        assert!(install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_needed_for_b0069_with_cli() {
        let tags = &["@B-0069", "@positive", "@cli"];
        assert!(install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_needed_for_b0070_with_cli() {
        let tags = &["@B-0070", "@positive", "@cli"];
        assert!(install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_not_needed_for_account_only() {
        let tags = &["@B-0043", "@positive", "@api"];
        assert!(!install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_not_needed_without_interface() {
        let tags = &["@B-0068", "@positive"];
        assert!(!install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_not_needed_without_behavior() {
        let tags = &["@positive", "@cli"];
        assert!(!install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_needed_for_b0068_with_api() {
        let tags = &["@B-0068", "@positive", "@api"];
        assert!(install_context_needed_from_tags(tags));
    }

    #[test]
    fn install_context_needed_for_b0069_with_mcp() {
        let tags = &["@B-0069", "@positive", "@mcp"];
        assert!(install_context_needed_from_tags(tags));
    }
}
