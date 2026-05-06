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
    InProcessHarness, McpHarness, StandardsInspectResult, TuiHarness, WebHarness,
};

/// Cucumber `World` shared across all Tanren BDD scenarios.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    pub seed: FixtureSeed,
    pub account: Option<AccountContext>,
    pub standards: Option<StandardsContext>,
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

pub struct StandardsContext {
    pub project_dir: PathBuf,
    pub last_result: Option<StandardsInspectResult>,
}

impl std::fmt::Debug for StandardsContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandardsContext")
            .field("project_dir", &self.project_dir)
            .field(
                "last_result_success",
                &self.last_result.as_ref().map(|r| r.success),
            )
            .finish_non_exhaustive()
    }
}

impl Drop for StandardsContext {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.project_dir);
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
            standards: None,
        };
        assert_eq!(world.seed.value(), 42);
    }
}
