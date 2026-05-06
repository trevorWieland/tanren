//! Per-interface BDD wire-harness wiring (R-0001 sub-9).
//!
//! Every account-flow BDD scenario tagged with one of the closed
//! interface tags (`@api`, `@cli`, `@mcp`, `@tui`, `@web`) routes
//! through the matching [`AccountHarness`] implementation rather than
//! calling `tanren_app_services::Handlers::*` directly.
//!
//! ## Status of each harness (PR 9)
//!
//! - `@api` — full impl. Spawns `tanren_api_app::build_app_with_store`
//!   on an ephemeral port, drives via `reqwest::Client` with
//!   `cookie_store(true)`.
//! - `@cli` — full impl. Spawns the `tanren-cli` binary via
//!   `tokio::process::Command` against a shared `SQLite` file.
//! - `@mcp` — full impl. Spawns `tanren_mcp_app::build_router_with_store`
//!   on an ephemeral port and drives the account-flow tools via
//!   the rmcp streamable-HTTP client.
//! - `@tui` — falls back to [`InProcessHarness`].
//! - `@web` — falls back to [`InProcessHarness`]. Playwright coverage
//!   runs in parallel on the Node side.
//! - untagged / fallback — [`InProcessHarness`].

mod api;
mod cli;
mod cli_parse;
mod in_process;
mod mcp;
mod shared;
mod tui;
mod types;
mod web;

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use secrecy::SecretString;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::ConfigurationFailureReason;
use tanren_identity_policy::AccountId;

pub use api::ApiHarness;
pub use cli::CliHarness;
pub use in_process::InProcessHarness;
pub use mcp::McpHarness;
pub use tui::TuiHarness;
pub use types::{
    ActorState, ConcurrentAcceptanceTally, HarnessConfigEntry, HarnessCredential, HarnessOutcome,
    event_kinds, record_failure,
};
pub use web::WebHarness;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    InProcess,
    Api,
    Cli,
    Mcp,
    Tui,
    Web,
}

impl HarnessKind {
    #[must_use]
    pub fn from_tags<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for tag in tags {
            let raw = tag.as_ref();
            let normalized = raw.strip_prefix('@').unwrap_or(raw);
            match normalized {
                "api" => return Self::Api,
                "cli" => return Self::Cli,
                "mcp" => return Self::Mcp,
                "tui" => return Self::Tui,
                "web" => return Self::Web,
                _ => {}
            }
        }
        Self::InProcess
    }
}

#[derive(Debug, Clone)]
pub struct HarnessSession {
    pub account: tanren_contract::AccountView,
    pub account_id: AccountId,
    pub expires_at: DateTime<Utc>,
    pub has_token: bool,
}

#[derive(Debug, Clone)]
pub struct HarnessAcceptance {
    pub session: HarnessSession,
    pub joined_org: tanren_identity_policy::OrgId,
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("{0:?}: {1}")]
    Account(tanren_contract::AccountFailureReason, String),
    #[error("{0:?}: {1}")]
    Configuration(ConfigurationFailureReason, String),
    #[error("transport: {0}")]
    Transport(String),
}

impl HarnessError {
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Account(reason, _) => reason.code().to_owned(),
            Self::Configuration(reason, _) => reason.code().to_owned(),
            Self::Transport(_) => "transport_error".to_owned(),
        }
    }
}

pub type HarnessResult<T> = Result<T, HarnessError>;

#[derive(Debug, Clone)]
pub struct HarnessInvitation {
    pub token: tanren_identity_policy::InvitationToken,
    pub inviting_org: tanren_identity_policy::OrgId,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AccountHarness: Send + std::fmt::Debug {
    fn kind(&self) -> HarnessKind;

    async fn sign_up(
        &mut self,
        req: tanren_contract::SignUpRequest,
    ) -> HarnessResult<HarnessSession>;

    async fn sign_in(
        &mut self,
        req: tanren_contract::SignInRequest,
    ) -> HarnessResult<HarnessSession>;

    async fn accept_invitation(
        &mut self,
        req: tanren_contract::AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance>;

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<tanren_contract::AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            out.push(self.accept_invitation(r).await);
        }
        out
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()>;

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<tanren_store::EventEnvelope>>;

    async fn set_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
        value: UserSettingValue,
    ) -> HarnessResult<HarnessConfigEntry> {
        let _ = (account_id, key, value);
        Err(HarnessError::Transport(
            "set_user_config not implemented for this harness".to_owned(),
        ))
    }

    async fn get_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let _ = (account_id, key);
        Err(HarnessError::Transport(
            "get_user_config not implemented for this harness".to_owned(),
        ))
    }

    async fn list_user_config(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessConfigEntry>> {
        let _ = account_id;
        Err(HarnessError::Transport(
            "list_user_config not implemented for this harness".to_owned(),
        ))
    }

    async fn attempt_get_other_user_config(
        &mut self,
        actor_account_id: AccountId,
        target_account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let _ = (actor_account_id, target_account_id, key);
        Err(HarnessError::Configuration(
            ConfigurationFailureReason::Unauthorized,
            "cross-account config read rejected".to_owned(),
        ))
    }

    async fn create_credential(
        &mut self,
        account_id: AccountId,
        kind: CredentialKind,
        name: String,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let _ = (account_id, kind, name, secret);
        Err(HarnessError::Transport(
            "create_credential not implemented for this harness".to_owned(),
        ))
    }

    async fn list_credentials(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessCredential>> {
        let _ = account_id;
        Err(HarnessError::Transport(
            "list_credentials not implemented for this harness".to_owned(),
        ))
    }

    async fn attempt_update_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let _ = (account_id, credential_id, secret);
        Err(HarnessError::Transport(
            "attempt_update_credential not implemented for this harness".to_owned(),
        ))
    }

    async fn attempt_remove_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
    ) -> HarnessResult<bool> {
        let _ = (account_id, credential_id);
        Err(HarnessError::Transport(
            "attempt_remove_credential not implemented for this harness".to_owned(),
        ))
    }
}

pub trait ConfigurationHarness: AccountHarness {}
impl<T: AccountHarness> ConfigurationHarness for T {}

pub(crate) const HARNESS_DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
