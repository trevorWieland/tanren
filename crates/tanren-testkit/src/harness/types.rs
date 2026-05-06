//! Harness-agnostic types shared across all wire-harness
//! implementations and BDD step definitions.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use secrecy;
use serde_json::Value;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, CredentialScope, UserSettingKey, UserSettingValue,
};
use tanren_contract::{AccountFailureReason, ConfigurationFailureReason};
use tanren_store::EventEnvelope;

use super::{HarnessError, HarnessSession};

/// Per-actor state captured by `Given <actor> has signed up ...` steps
/// so subsequent steps can sign them in or assert on the prior outcome.
#[derive(Debug, Default, Clone)]
pub struct ActorState {
    pub identifier: Option<String>,
    pub password: Option<secrecy::SecretString>,
    pub sign_up: Option<HarnessSession>,
    pub sign_in: Option<HarnessSession>,
    pub accept_invitation: Option<super::HarnessAcceptance>,
    pub last_failure: Option<AccountFailureReason>,
    pub last_config_failure: Option<ConfigurationFailureReason>,
    pub config_entries: Vec<HarnessConfigEntry>,
    pub credentials: Vec<HarnessCredential>,
}

/// Outcome of the most recent action.
#[derive(Debug, Clone)]
pub enum HarnessOutcome {
    SignedUp(HarnessSession),
    SignedIn(HarnessSession),
    AcceptedInvitation(super::HarnessAcceptance),
    Failure(AccountFailureReason),
    ConfigFailure(ConfigurationFailureReason),
    Other(String),
}

impl HarnessOutcome {
    /// Project the failure code for this outcome.
    #[must_use]
    pub fn failure_code(&self) -> Option<String> {
        match self {
            Self::Failure(reason) => Some(reason.code().to_owned()),
            Self::ConfigFailure(reason) => Some(reason.code().to_owned()),
            Self::SignedUp(_)
            | Self::SignedIn(_)
            | Self::AcceptedInvitation(_)
            | Self::Other(_) => None,
        }
    }
}

/// Redacted credential metadata returned by the harness.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HarnessCredential {
    pub id: CredentialId,
    pub name: String,
    pub kind: CredentialKind,
    pub scope: CredentialScope,
    pub present: bool,
}

/// A single user-tier configuration key-value pair.
#[derive(Debug, Clone)]
pub struct HarnessConfigEntry {
    pub key: UserSettingKey,
    pub value: UserSettingValue,
    pub updated_at: DateTime<Utc>,
}

/// Project a [`HarnessError`] into the actor-state + outcome pair.
pub fn record_failure(err: HarnessError, entry: &mut ActorState) -> HarnessOutcome {
    match err {
        HarnessError::Account(reason, _) => {
            entry.last_failure = Some(reason);
            HarnessOutcome::Failure(reason)
        }
        HarnessError::Configuration(reason, _) => {
            entry.last_config_failure = Some(reason);
            HarnessOutcome::ConfigFailure(reason)
        }
        HarnessError::Transport(message) => HarnessOutcome::Other(format!("transport: {message}")),
    }
}

/// Filter `recent_events` rows by their `payload.kind` field.
#[must_use]
pub fn event_kinds(events: &[EventEnvelope]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| {
            e.payload
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// Track concurrent invitation-acceptance outcomes for the
/// falsification race scenario.
#[derive(Debug, Default)]
pub struct ConcurrentAcceptanceTally {
    pub successes: usize,
    pub failures_by_code: HashMap<String, usize>,
    pub other: Vec<String>,
}

impl ConcurrentAcceptanceTally {
    pub fn record(&mut self, outcome: Result<super::HarnessAcceptance, HarnessError>) {
        match outcome {
            Ok(_) => self.successes += 1,
            Err(HarnessError::Account(reason, _)) => {
                let code = reason.code().to_owned();
                *self.failures_by_code.entry(code).or_insert(0) += 1;
            }
            Err(HarnessError::Configuration(reason, _)) => {
                let code = reason.code().to_owned();
                *self.failures_by_code.entry(code).or_insert(0) += 1;
            }
            Err(HarnessError::Transport(msg)) => self.other.push(msg),
        }
    }

    #[must_use]
    pub fn failures_with_code(&self, code: &str) -> usize {
        self.failures_by_code.get(code).copied().unwrap_or(0)
    }
}
