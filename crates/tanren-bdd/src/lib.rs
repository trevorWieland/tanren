//! BDD step-definition home for Tanren.
//!
//! This is the only crate in the workspace permitted to define `#[test]`
//! items — `xtask check-rust-test-surface` mechanically rejects them
//! anywhere else. R-0001 (S-10) lands the first feature
//! (`B-0043-create-account.feature`) and the supporting account-flow
//! step definitions.

pub mod steps;

use cucumber::World as CucumberWorld;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use tanren_app_services::{Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationResponse, AccountFailureReason, SignInResponse, SignUpResponse,
};
use tanren_identity_policy::Argon2idVerifier;
use tanren_testkit::{FixtureSeed, InvitationFixture};

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
            let ctx = AccountContext::new()
                .await
                .expect("ephemeral SQLite should connect for BDD");
            self.account = Some(ctx);
        }
        self.account
            .as_mut()
            .expect("account context just initialized")
    }
}

/// Per-scenario in-memory state for the account-flow steps.
#[derive(Debug)]
pub struct AccountContext {
    /// Shared store.
    pub store: Store,
    /// Handler facade.
    pub handlers: Handlers,
    /// Mutable shared clock — scenarios can warp time forward to expire
    /// invitations.
    pub clock: SharedClock,
    /// Registry of actors by display name.
    pub actors: HashMap<String, ActorState>,
    /// Pending invitations seeded by the scenario, keyed by token.
    pub invitations: HashMap<String, InvitationFixture>,
    /// The most recent action's outcome.
    pub last_outcome: Option<Outcome>,
}

impl AccountContext {
    /// Construct a fresh context backed by an in-memory `SQLite` store.
    ///
    /// # Errors
    ///
    /// Returns the underlying store error if connection or migration fails.
    pub async fn new() -> Result<Self, tanren_store::StoreError> {
        let store = tanren_testkit::ephemeral_store().await?;
        let clock = SharedClock::new(Utc::now());
        let handlers = Handlers::with_verifier(
            clock.as_app_clock(),
            Arc::new(Argon2idVerifier::fast_for_tests()),
        );
        Ok(Self {
            store,
            handlers,
            clock,
            actors: HashMap::new(),
            invitations: HashMap::new(),
            last_outcome: None,
        })
    }
}

/// Per-actor state captured by `Given <actor> has signed up ...` style
/// steps so subsequent steps can sign them in or assert membership.
#[derive(Debug, Clone, Default)]
pub struct ActorState {
    /// Identifier (email) the actor signed up with.
    pub identifier: Option<String>,
    /// Password the actor signed up with.
    pub password: Option<String>,
    /// Last successful sign-up response, if any.
    pub sign_up: Option<SignUpResponse>,
    /// Last successful sign-in response, if any.
    pub sign_in: Option<SignInResponse>,
    /// Last successful invitation acceptance, if any.
    pub accept_invitation: Option<AcceptInvitationResponse>,
    /// Last failure (taxonomy code), if any.
    pub last_failure: Option<AccountFailureReason>,
}

/// Outcome of the most recent account-flow action.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Successful sign-up.
    SignedUp(SignUpResponse),
    /// Successful sign-in.
    SignedIn(SignInResponse),
    /// Successful invitation acceptance.
    AcceptedInvitation(AcceptInvitationResponse),
    /// Account-flow taxonomy failure.
    Failure(AccountFailureReason),
    /// Non-taxonomy infrastructure failure.
    Other(String),
}

/// Mutable clock shared between scenarios and the [`Handlers`] facade.
#[derive(Debug, Clone)]
pub struct SharedClock {
    inner: Arc<Mutex<DateTime<Utc>>>,
}

impl SharedClock {
    /// Build a new shared clock that initially reports `now`.
    #[must_use]
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(now)),
        }
    }

    /// Read the current clock instant.
    #[must_use]
    pub fn read(&self) -> DateTime<Utc> {
        *self.inner.lock().expect("shared clock mutex poisoned")
    }

    /// Set the clock to a specific instant.
    pub fn set(&self, when: DateTime<Utc>) {
        *self.inner.lock().expect("shared clock mutex poisoned") = when;
    }

    /// Wrap as an `app_services::Clock`.
    #[must_use]
    pub fn as_app_clock(&self) -> Clock {
        let inner = self.inner.clone();
        Clock::from_fn(move || *inner.lock().expect("shared clock mutex poisoned"))
    }
}

/// Run the cucumber harness against the supplied features directory.
pub async fn run_features(features_dir: impl Into<PathBuf>) {
    TanrenWorld::cucumber()
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
        };
        assert_eq!(world.seed.value(), 42);
    }
}
