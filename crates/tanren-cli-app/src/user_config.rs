use std::io::Write;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Subcommand, ValueEnum};
use secrecy::SecretString;
use tanren_app_services::{AccountStore, Handlers, Store};
use tanren_configuration_secrets::{
    OwnerScope, ThemePreference, UserCredentialKind, UserCredentialStatus, UserSettingKey,
    UserSettingValue,
};
use tanren_contract::{
    CreateUserCredentialRequest, UpdateUserCredentialRequest, UpsertUserSettingRequest,
    UserCredentialView,
};
use tanren_identity_policy::AccountId;
use tanren_store::SessionAuthenticationLookup;
use uuid::Uuid;

use crate::{account_error, load_persisted_session_token};

const ACCOUNT_ID_ENV: &str = "TANREN_ACCOUNT_ID";

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
    /// User-tier configuration operations.
    User {
        #[command(subcommand)]
        action: UserConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum UserConfigAction {
    /// List user-tier settings for the authenticated account.
    List {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Target account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
    },
    /// Upsert one user-tier setting.
    Set {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Target account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        /// Setting key.
        #[arg(long, value_enum)]
        key: CliUserSettingKey,
        /// Setting value.
        #[arg(long)]
        value: String,
    },
    /// Remove one user-tier setting.
    Remove {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Target account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        /// Setting key.
        #[arg(long, value_enum)]
        key: CliUserSettingKey,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CredentialAction {
    /// Add one user-owned credential.
    Add {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        /// Credential kind.
        #[arg(long, value_enum)]
        kind: CliCredentialKind,
    },
    /// Update one user-owned credential value.
    Update {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        /// Credential metadata id.
        #[arg(long)]
        item_id: String,
    },
    /// List user-owned credential metadata.
    List {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
    },
    /// Remove one user-owned credential.
    Remove {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        /// Credential metadata id.
        #[arg(long)]
        item_id: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CliUserSettingKey {
    Theme,
    Editor,
}

impl From<CliUserSettingKey> for UserSettingKey {
    fn from(value: CliUserSettingKey) -> Self {
        match value {
            CliUserSettingKey::Theme => Self::Theme,
            CliUserSettingKey::Editor => Self::Editor,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CliCredentialKind {
    ProviderApiToken,
    HarnessApiToken,
}

impl From<CliCredentialKind> for UserCredentialKind {
    fn from(value: CliCredentialKind) -> Self {
        match value {
            CliCredentialKind::ProviderApiToken => Self::ProviderApiToken,
            CliCredentialKind::HarnessApiToken => Self::HarnessApiToken,
        }
    }
}

pub(crate) fn dispatch_config(action: ConfigAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_config(action))
}

pub(crate) fn dispatch_credential(action: CredentialAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_credential(action))
}

async fn run_config(action: ConfigAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        ConfigAction::User { action } => match action {
            UserConfigAction::List {
                database_url,
                account_id,
            } => {
                let (store, authenticated_account_id) =
                    connect_store_and_authenticate(&database_url).await?;
                let requested_account_id = parse_account_id(&account_id)?;
                let response = handlers
                    .list_user_settings(&store, authenticated_account_id, requested_account_id)
                    .await
                    .map_err(account_error)?;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                for item in response.items {
                    writeln!(
                        handle,
                        "setting key={} value={} updated_at={}",
                        setting_key_name(item.key),
                        setting_value_name(item.value),
                        item.updated_at.to_rfc3339()
                    )
                    .context("write user-setting list row")?;
                }
            }
            UserConfigAction::Set {
                database_url,
                account_id,
                key,
                value,
            } => {
                let (store, authenticated_account_id) =
                    connect_store_and_authenticate(&database_url).await?;
                let requested_account_id = parse_account_id(&account_id)?;
                let request = UpsertUserSettingRequest {
                    key: key.into(),
                    value: parse_setting_value(key, &value)?,
                };
                let response = handlers
                    .upsert_user_setting(
                        &store,
                        authenticated_account_id,
                        requested_account_id,
                        request,
                    )
                    .await
                    .map_err(account_error)?;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(
                    handle,
                    "setting key={} value={} updated_at={}",
                    setting_key_name(response.setting.key),
                    setting_value_name(response.setting.value),
                    response.setting.updated_at.to_rfc3339()
                )
                .context("write upsert user-setting result")?;
            }
            UserConfigAction::Remove {
                database_url,
                account_id,
                key,
            } => {
                let (store, authenticated_account_id) =
                    connect_store_and_authenticate(&database_url).await?;
                let requested_account_id = parse_account_id(&account_id)?;
                let response = handlers
                    .remove_user_setting(
                        &store,
                        authenticated_account_id,
                        requested_account_id,
                        key.into(),
                    )
                    .await
                    .map_err(account_error)?;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(
                    handle,
                    "removed key={} value={} updated_at={}",
                    setting_key_name(response.setting.key),
                    setting_value_name(response.setting.value),
                    response.setting.updated_at.to_rfc3339()
                )
                .context("write remove user-setting result")?;
            }
        },
    }
    Ok(())
}

async fn run_credential(action: CredentialAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        CredentialAction::Add {
            database_url,
            account_id,
            kind,
        } => {
            let (store, authenticated_account_id) =
                connect_store_and_authenticate(&database_url).await?;
            let requested_account_id = parse_account_id(&account_id)?;
            let value = read_secret_from_stdin()?;
            let response = handlers
                .add_user_credential(
                    &store,
                    authenticated_account_id,
                    CreateUserCredentialRequest {
                        kind: kind.into(),
                        owner_scope: OwnerScope::User {
                            account_id: requested_account_id,
                        },
                        value,
                    },
                )
                .await
                .map_err(account_error)?;
            print_credential_row("credential", &response.item)?;
        }
        CredentialAction::Update {
            database_url,
            account_id,
            item_id,
        } => {
            let (store, authenticated_account_id) =
                connect_store_and_authenticate(&database_url).await?;
            let requested_account_id = parse_account_id(&account_id)?;
            let value = read_secret_from_stdin()?;
            let response = handlers
                .update_user_credential(
                    &store,
                    authenticated_account_id,
                    &item_id,
                    OwnerScope::User {
                        account_id: requested_account_id,
                    },
                    UpdateUserCredentialRequest { value },
                )
                .await
                .map_err(account_error)?;
            print_credential_row("credential", &response.item)?;
        }
        CredentialAction::List {
            database_url,
            account_id,
        } => {
            let (store, authenticated_account_id) =
                connect_store_and_authenticate(&database_url).await?;
            let requested_account_id = parse_account_id(&account_id)?;
            let response = handlers
                .list_user_credentials(
                    &store,
                    authenticated_account_id,
                    OwnerScope::User {
                        account_id: requested_account_id,
                    },
                )
                .await
                .map_err(account_error)?;
            for item in response.items {
                print_credential_row("credential", &item)?;
            }
        }
        CredentialAction::Remove {
            database_url,
            account_id,
            item_id,
        } => {
            let (store, authenticated_account_id) =
                connect_store_and_authenticate(&database_url).await?;
            let requested_account_id = parse_account_id(&account_id)?;
            let response = handlers
                .remove_user_credential(
                    &store,
                    authenticated_account_id,
                    &item_id,
                    OwnerScope::User {
                        account_id: requested_account_id,
                    },
                )
                .await
                .map_err(account_error)?;
            print_credential_row("removed", &response.item)?;
        }
    }
    Ok(())
}

async fn resolve_authenticated_account_id(store: &Store) -> Result<AccountId> {
    let session_token = load_persisted_session_token()?;
    let authenticated = store
        .authenticate_session(SessionAuthenticationLookup {
            session_token,
            now: Utc::now(),
        })
        .await
        .context("authenticate persisted session token")?;
    authenticated
        .ok_or_else(|| {
            anyhow::anyhow!(
                "error: authentication_required — sign in first (`tanren-cli account sign-in`)."
            )
        })
        .map(|session| session.authenticated_account_id)
}

async fn connect_store_and_authenticate(database_url: &str) -> Result<(Store, AccountId)> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let authenticated_account_id = resolve_authenticated_account_id(&store).await?;
    Ok((store, authenticated_account_id))
}

fn parse_account_id(raw: &str) -> Result<AccountId> {
    let parsed = Uuid::parse_str(raw).with_context(|| format!("parse account id {raw}"))?;
    Ok(AccountId::new(parsed))
}

fn read_secret_from_stdin() -> Result<SecretString> {
    let mut raw = String::new();
    std::io::stdin()
        .read_line(&mut raw)
        .context("read credential value from stdin")?;
    let value = raw.trim_end_matches(['\n', '\r']).to_owned();
    if value.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "credential value from stdin cannot be empty or whitespace"
        ));
    }
    Ok(SecretString::from(value))
}

fn parse_setting_value(key: CliUserSettingKey, raw: &str) -> Result<UserSettingValue> {
    match key {
        CliUserSettingKey::Theme => {
            let value = match raw {
                "system" => ThemePreference::System,
                "light" => ThemePreference::Light,
                "dark" => ThemePreference::Dark,
                _ => {
                    return Err(anyhow::anyhow!(
                        "invalid --value for theme; expected one of: system, light, dark"
                    ));
                }
            };
            Ok(UserSettingValue::Theme(value))
        }
        CliUserSettingKey::Editor => Ok(UserSettingValue::Editor(raw.to_owned())),
    }
}

fn setting_key_name(key: UserSettingKey) -> &'static str {
    match key {
        UserSettingKey::Theme => "theme",
        UserSettingKey::Editor => "editor",
    }
}

fn setting_value_name(value: UserSettingValue) -> String {
    match value {
        UserSettingValue::Theme(pref) => match pref {
            ThemePreference::System => "theme:system".to_owned(),
            ThemePreference::Light => "theme:light".to_owned(),
            ThemePreference::Dark => "theme:dark".to_owned(),
        },
        UserSettingValue::Editor(editor) => format!("editor:{editor}"),
    }
}

fn print_credential_row(prefix: &str, item: &UserCredentialView) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "{prefix} id={} kind={} scope=user:{} status={} created_at={} updated_at={}",
        item.id,
        credential_kind_name(item.kind),
        owner_account_id(item.owner_scope),
        credential_status_name(item.status),
        item.created_at.to_rfc3339(),
        item.updated_at.to_rfc3339()
    )
    .with_context(|| format!("write {prefix} row"))?;
    Ok(())
}

fn owner_account_id(scope: OwnerScope) -> AccountId {
    match scope {
        OwnerScope::User { account_id } => account_id,
    }
}

fn credential_kind_name(kind: UserCredentialKind) -> &'static str {
    match kind {
        UserCredentialKind::ProviderApiToken => "provider_api_token",
        UserCredentialKind::HarnessApiToken => "harness_api_token",
    }
}

fn credential_status_name(status: UserCredentialStatus) -> &'static str {
    match status {
        UserCredentialStatus::Pending => "pending",
        UserCredentialStatus::Active => "active",
        UserCredentialStatus::Invalid => "invalid",
    }
}
