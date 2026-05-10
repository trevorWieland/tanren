use crate::{account_error, load_persisted_session_token};
use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Subcommand, ValueEnum};
use secrecy::SecretString;
use std::io::Write;
use tanren_app_services::{Handlers, SessionAuthenticationRequest, Store};
use tanren_contract::{
    CreateUserCredentialRequest, EditorSetting, OwnerScope, SUPPORTED_THEME_PREFERENCES,
    UpdateUserCredentialRequest, UpsertUserSettingRequest, UserCredentialId, UserCredentialKind,
    UserCredentialView, UserSettingKey, UserSettingValue, parse_theme_preference,
    theme_preference_name, user_credential_kind_name, user_credential_status_name,
    user_credentials_page_request, user_setting_key_name, user_settings_page_request,
};
use tanren_identity_policy::AccountId;
use uuid::Uuid;
use zeroize::Zeroizing;
const ACCOUNT_ID_ENV: &str = "TANREN_ACCOUNT_ID";
#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
    User {
        #[command(subcommand)]
        action: UserConfigAction,
    },
}
#[derive(Debug, Subcommand)]
pub(crate) enum UserConfigAction {
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        #[arg(long)]
        limit: Option<u16>,
        #[arg(long)]
        after: Option<String>,
    },
    Set {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        #[arg(long, value_enum)]
        key: CliUserSettingKey,
        #[arg(long)]
        value: String,
    },
    Remove {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        #[arg(long, value_enum)]
        key: CliUserSettingKey,
    },
}
#[derive(Debug, Subcommand)]
pub(crate) enum CredentialAction {
    Add {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        #[arg(long, value_enum)]
        kind: CliCredentialKind,
    },
    Update {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        #[arg(long)]
        item_id: String,
    },
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
        #[arg(long)]
        limit: Option<u16>,
        #[arg(long)]
        after: Option<String>,
    },
    Remove {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, env = ACCOUNT_ID_ENV)]
        account_id: String,
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
                limit,
                after,
            } => run_user_config_list(&handlers, &database_url, &account_id, limit, after).await?,
            UserConfigAction::Set {
                database_url,
                account_id,
                key,
                value,
            } => run_user_config_set(&handlers, &database_url, &account_id, key, value).await?,
            UserConfigAction::Remove {
                database_url,
                account_id,
                key,
            } => run_user_config_remove(&handlers, &database_url, &account_id, key).await?,
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
        } => run_credential_add(&handlers, &database_url, &account_id, kind).await?,
        CredentialAction::Update {
            database_url,
            account_id,
            item_id,
        } => run_credential_update(&handlers, &database_url, &account_id, &item_id).await?,
        CredentialAction::List {
            database_url,
            account_id,
            limit,
            after,
        } => run_credential_list(&handlers, &database_url, &account_id, limit, after).await?,
        CredentialAction::Remove {
            database_url,
            account_id,
            item_id,
        } => run_credential_remove(&handlers, &database_url, &account_id, &item_id).await?,
    }
    Ok(())
}
async fn run_user_config_list(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    limit: Option<u16>,
    after: Option<String>,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
    let response = handlers
        .list_user_settings_page(
            &store,
            authenticated_account_id,
            requested_account_id,
            user_settings_page_request(limit, after),
        )
        .await
        .map_err(account_error)?;
    let mut handle = std::io::stdout().lock();
    for item in response.items {
        writeln!(
            handle,
            "setting key={} value={} updated_at={}",
            user_setting_key_name(item.key),
            setting_value_name(item.value),
            item.updated_at.to_rfc3339()
        )
        .context("write user-setting list row")?;
    }
    if let Some(next_cursor) = response.next_cursor {
        writeln!(handle, "next_cursor={next_cursor}").context("write user-setting next cursor")?;
    }
    Ok(())
}
async fn run_user_config_set(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    key: CliUserSettingKey,
    value: String,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
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
    let mut handle = std::io::stdout().lock();
    writeln!(
        handle,
        "setting key={} value={} updated_at={}",
        user_setting_key_name(response.setting.key),
        setting_value_name(response.setting.value),
        response.setting.updated_at.to_rfc3339()
    )
    .context("write upsert user-setting result")?;
    Ok(())
}
async fn run_user_config_remove(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    key: CliUserSettingKey,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
    let response = handlers
        .remove_user_setting(
            &store,
            authenticated_account_id,
            requested_account_id,
            key.into(),
        )
        .await
        .map_err(account_error)?;
    let mut handle = std::io::stdout().lock();
    writeln!(
        handle,
        "removed key={} value={} updated_at={}",
        user_setting_key_name(response.setting.key),
        setting_value_name(response.setting.value),
        response.setting.updated_at.to_rfc3339()
    )
    .context("write remove user-setting result")?;
    Ok(())
}
async fn run_credential_add(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    kind: CliCredentialKind,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
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
    Ok(())
}
async fn run_credential_update(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    item_id: &str,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
    let item_id = parse_credential_id(item_id)?;
    let value = read_secret_from_stdin()?;
    let response = handlers
        .update_user_credential(
            &store,
            authenticated_account_id,
            item_id,
            OwnerScope::User {
                account_id: requested_account_id,
            },
            UpdateUserCredentialRequest { value },
        )
        .await
        .map_err(account_error)?;
    print_credential_row("credential", &response.item)?;
    Ok(())
}
async fn run_credential_list(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    limit: Option<u16>,
    after: Option<String>,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
    let response = handlers
        .list_user_credentials_page(
            &store,
            authenticated_account_id,
            OwnerScope::User {
                account_id: requested_account_id,
            },
            user_credentials_page_request(limit, after),
        )
        .await
        .map_err(account_error)?;
    for item in response.items {
        print_credential_row("credential", &item)?;
    }
    if let Some(next_cursor) = response.next_cursor {
        let mut handle = std::io::stdout().lock();
        writeln!(handle, "next_cursor={next_cursor}").context("write credential next cursor")?;
    }
    Ok(())
}
async fn run_credential_remove(
    handlers: &Handlers,
    database_url: &str,
    account_id: &str,
    item_id: &str,
) -> Result<()> {
    let (store, authenticated_account_id) = connect_store_and_authenticate(database_url).await?;
    let requested_account_id = parse_account_id(account_id)?;
    let item_id = parse_credential_id(item_id)?;
    let response = handlers
        .remove_user_credential(
            &store,
            authenticated_account_id,
            item_id,
            OwnerScope::User {
                account_id: requested_account_id,
            },
        )
        .await
        .map_err(account_error)?;
    print_credential_row("removed", &response.item)?;
    Ok(())
}
async fn resolve_authenticated_account_id(store: &Store) -> Result<AccountId> {
    let handlers = Handlers::new();
    let session_token = load_persisted_session_token()?;
    let authenticated = handlers
        .authenticate_session(
            store,
            SessionAuthenticationRequest {
                session_token,
                now: Utc::now(),
            },
        )
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
fn parse_credential_id(raw: &str) -> Result<UserCredentialId> {
    UserCredentialId::parse(raw).with_context(|| format!("parse credential id {raw}"))
}
fn read_secret_from_stdin() -> Result<SecretString> {
    let mut raw = Zeroizing::new(String::new());
    std::io::stdin()
        .read_line(&mut raw)
        .context("read credential value from stdin")?;
    while raw.ends_with('\n') || raw.ends_with('\r') {
        raw.pop();
    }
    if raw.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "credential value from stdin cannot be empty or whitespace"
        ));
    }
    Ok(SecretString::from(std::mem::take(&mut *raw)))
}
fn parse_setting_value(key: CliUserSettingKey, raw: &str) -> Result<UserSettingValue> {
    match key {
        CliUserSettingKey::Theme => {
            let value = parse_theme_preference(raw).ok_or_else(|| {
                anyhow::anyhow!(
                    "invalid --value for theme; expected one of: {}",
                    SUPPORTED_THEME_PREFERENCES.join(", ")
                )
            })?;
            Ok(UserSettingValue::Theme(value))
        }
        CliUserSettingKey::Editor => Ok(UserSettingValue::Editor(EditorSetting::from_unvalidated(
            raw.to_owned(),
        ))),
    }
}
fn setting_value_name(value: UserSettingValue) -> String {
    match value {
        UserSettingValue::Theme(pref) => format!("theme:{}", theme_preference_name(pref)),
        UserSettingValue::Editor(editor) => format!("editor:{}", editor.value()),
    }
}
fn print_credential_row(prefix: &str, item: &UserCredentialView) -> Result<()> {
    let mut handle = std::io::stdout().lock();
    writeln!(
        handle,
        "{prefix} id={} kind={} scope=user:{} status={} created_at={} updated_at={}",
        item.id,
        user_credential_kind_name(item.kind),
        owner_account_id(item.owner_scope),
        user_credential_status_name(item.status),
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
