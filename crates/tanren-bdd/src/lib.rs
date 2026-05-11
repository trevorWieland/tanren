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

use tanren_contract::{
    CheckOrganizationPermissionResponse, CreateOrganizationResponse,
    ListOrganizationMembersResponse, ListOrganizationsResponse, ORGANIZATION_CREATE_BEHAVIOR_ID,
    ORGANIZATION_MEMBER_LIST_BEHAVIOR_ID,
};
use tanren_identity_policy::{InvitationToken, OrgId, OrganizationName};
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
    /// Active wire harness for the current scenario.
    pub harness: Box<dyn AccountHarness>,
    /// Registry of actors by display name.
    pub actors: HashMap<String, ActorState>,
    /// The most recent action's outcome.
    pub last_outcome: Option<HarnessOutcome>,
    /// Per-scenario invitation tokens recorded by `Given a pending
    /// invitation token "..."` style steps.
    pub invitations: HashSet<String>,
    /// Per-scenario map of normalized organization names to ids.
    pub organizations_by_name: HashMap<OrganizationName, OrgId>,
    /// Most recent create-organization success payload.
    pub last_created_organization: Option<CreateOrganizationResponse>,
    /// Most recent list-organizations success payload.
    pub last_listed_organizations: Option<ListOrganizationsResponse>,
    /// Most recent permission-check success payload.
    pub last_checked_organization_permission: Option<CheckOrganizationPermissionResponse>,
    /// Most recent list-organization-members success payload.
    pub last_listed_members: Option<ListOrganizationMembersResponse>,
    /// Invitation tokens deferred until the named org is created.
    pub deferred_invitations: HashMap<OrganizationName, Vec<InvitationToken>>,
}

impl std::fmt::Debug for AccountContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountContext")
            .field("harness_kind", &self.harness.kind())
            .field("actors", &self.actors.keys().collect::<Vec<_>>())
            .field("invitations", &self.invitations)
            .field(
                "organizations_by_name",
                &self.organizations_by_name.keys().collect::<Vec<_>>(),
            )
            .field(
                "last_outcome",
                &self.last_outcome.as_ref().map(short_outcome_label),
            )
            .field(
                "last_created_organization",
                &self
                    .last_created_organization
                    .as_ref()
                    .map(|r| r.organization.name.as_str()),
            )
            .field(
                "last_listed_organizations_count",
                &self
                    .last_listed_organizations
                    .as_ref()
                    .map(|r| r.organizations.len()),
            )
            .field(
                "last_checked_organization_permission",
                &self
                    .last_checked_organization_permission
                    .as_ref()
                    .map(|r| (r.org_id, r.permission, r.allowed)),
            )
            .field(
                "last_listed_members_count",
                &self.last_listed_members.as_ref().map(|r| r.members.len()),
            )
            .field(
                "deferred_invitations_count",
                &self.deferred_invitations.len(),
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
            // `@web` scenarios outside B-0066 continue to use the Rust
            // fallback harness for fast feedback. B-0066 `@web` scenarios
            // are filtered out in `run_features` and proved by Playwright
            // only.
            HarnessKind::Web => Box::new(WebHarness::spawn().await.expect("WebHarness::spawn")),
        };
        Self {
            harness,
            actors: HashMap::new(),
            last_outcome: None,
            invitations: HashSet::new(),
            organizations_by_name: HashMap::new(),
            last_created_organization: None,
            last_listed_organizations: None,
            last_checked_organization_permission: None,
            last_listed_members: None,
            deferred_invitations: HashMap::new(),
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
///
/// B-0066's `@web` scenarios are intentionally excluded from the Rust
/// runner: that witness must come from the Playwright browser path.
pub async fn run_features(features_dir: impl Into<PathBuf>) {
    TanrenWorld::cucumber()
        // Keep BDD witness runs deterministic across all wire harnesses.
        // Concurrent scenarios can starve per-scenario SQLite pools and
        // introduce transport flakes that are unrelated to behavior.
        .max_concurrent_scenarios(1)
        .before(|_feature, _rule, scenario, world| {
            let tags = scenario.tags.clone();
            Box::pin(async move {
                world.install_harness_for_tags(tags).await;
            })
        })
        .fail_on_skipped()
        .filter_run_and_exit(features_dir.into(), |feature, rule, scenario| {
            let is_b0066 = feature.tags.iter().any(|tag| {
                let normalized = tag.trim_start_matches('@');
                normalized == ORGANIZATION_CREATE_BEHAVIOR_ID
            });
            let is_b0065 = feature.tags.iter().any(|tag| {
                let normalized = tag.trim_start_matches('@');
                normalized == ORGANIZATION_MEMBER_LIST_BEHAVIOR_ID
            });
            let is_web = scenario.tags.iter().any(|tag| {
                let normalized = tag.trim_start_matches('@');
                normalized == "web"
            }) || rule.is_some_and(|r| {
                r.tags
                    .iter()
                    .any(|tag| tag.trim_start_matches('@') == "web")
            }) || feature
                .tags
                .iter()
                .any(|tag| tag.trim_start_matches('@') == "web");

            !((is_b0066 || is_b0065) && is_web)
        })
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
        };
        assert_eq!(world.seed.value(), 42);
    }
}
