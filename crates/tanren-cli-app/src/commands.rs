use std::fs;
use std::io::Write;

use anyhow::{Context, Result};
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, AuthenticatedActor, Handlers, Store};
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, RedactedCredentialMetadata, UserSettingKey, UserSettingValue,
};
use tanren_contract::{
    AcceptInvitationRequest, CreateCredentialRequest, RemoveCredentialRequest,
    RemoveUserConfigRequest, SetUserConfigRequest, SignInRequest, SignUpRequest,
    UpdateCredentialRequest,
};
use tanren_identity_policy::{Email, InvitationToken, SessionToken};
use uuid::Uuid;

use crate::{AccountAction, CredentialAction, UserConfigAction, persist_session, session_path};

pub(super) fn dispatch_account(action: AccountAction) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    rt.block_on(run_account(action))
}

async fn run_account(action: AccountAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        AccountAction::Create {
            database_url,
            identifier,
            password,
            display_name,
            invitation,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let email = Email::parse(&identifier).context("parse --identifier as email")?;
            let password = SecretString::from(password);
            match invitation {
                None => {
                    let resp = handlers
                        .sign_up(
                            &store,
                            SignUpRequest {
                                email,
                                password,
                                display_name,
                            },
                        )
                        .await
                        .map_err(app_service_error)?;
                    persist_session(resp.session.token.expose_secret())?;
                    writeln!(
                        std::io::stdout().lock(),
                        "account_id={id} session={token}",
                        id = resp.account.id,
                        token = resp.session.token.expose_secret(),
                    )
                    .context("write sign-up result")?;
                }
                Some(token) => {
                    let it = InvitationToken::parse(&token).context("parse --invitation")?;
                    let resp = handlers
                        .accept_invitation(
                            &store,
                            AcceptInvitationRequest {
                                invitation_token: it,
                                email,
                                password,
                                display_name,
                            },
                        )
                        .await
                        .map_err(app_service_error)?;
                    persist_session(resp.session.token.expose_secret())?;
                    writeln!(
                        std::io::stdout().lock(),
                        "account_id={id} session={token} joined_org={org}",
                        id = resp.account.id,
                        token = resp.session.token.expose_secret(),
                        org = resp.joined_org,
                    )
                    .context("write invitation-acceptance result")?;
                }
            }
        }
        AccountAction::SignIn {
            database_url,
            identifier,
            password,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let email = Email::parse(&identifier).context("parse --identifier as email")?;
            let password = SecretString::from(password);
            let resp = handlers
                .sign_in(&store, SignInRequest { email, password })
                .await
                .map_err(app_service_error)?;
            persist_session(resp.session.token.expose_secret())?;
            writeln!(
                std::io::stdout().lock(),
                "account_id={id} session={token}",
                id = resp.account.id,
                token = resp.session.token.expose_secret(),
            )
            .context("write sign-in result")?;
        }
    }
    Ok(())
}

pub(super) fn dispatch_user_config(action: UserConfigAction) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    rt.block_on(run_user_config(action))
}

async fn run_user_config(action: UserConfigAction) -> Result<()> {
    let db = match &action {
        UserConfigAction::List { database_url }
        | UserConfigAction::Set { database_url, .. }
        | UserConfigAction::Remove { database_url, .. } => database_url,
    };
    let (handlers, store, actor) = connect_and_authenticate(db).await?;
    match action {
        UserConfigAction::List { .. } => {
            let resp = handlers
                .list_user_config(&store, &actor)
                .await
                .map_err(app_service_error)?;
            let mut h = std::io::stdout().lock();
            for e in &resp.entries {
                writeln!(
                    h,
                    "key={} value={} updated_at={}",
                    e.key, e.value, e.updated_at
                )
                .context("write config entry")?;
            }
        }
        UserConfigAction::Set { key, value, .. } => {
            let key = parse_setting_key(&key)?;
            let value = UserSettingValue::parse(&value).context("validate --value")?;
            let resp = handlers
                .set_user_config(&store, &actor, SetUserConfigRequest { key, value })
                .await
                .map_err(app_service_error)?;
            writeln!(
                std::io::stdout().lock(),
                "key={} value={} updated_at={}",
                resp.entry.key,
                resp.entry.value,
                resp.entry.updated_at
            )
            .context("write set result")?;
        }
        UserConfigAction::Remove { key, .. } => {
            let key = parse_setting_key(&key)?;
            let resp = handlers
                .remove_user_config(&store, &actor, RemoveUserConfigRequest { key })
                .await
                .map_err(app_service_error)?;
            writeln!(std::io::stdout().lock(), "removed={}", resp.removed)
                .context("write remove result")?;
        }
    }
    Ok(())
}

pub(super) fn dispatch_credential(action: CredentialAction) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    rt.block_on(run_credential(action))
}

async fn run_credential(action: CredentialAction) -> Result<()> {
    let db = match &action {
        CredentialAction::Add { database_url, .. }
        | CredentialAction::Update { database_url, .. }
        | CredentialAction::List { database_url }
        | CredentialAction::Remove { database_url, .. } => database_url,
    };
    let (handlers, store, actor) = connect_and_authenticate(db).await?;
    match action {
        CredentialAction::List { .. } => {
            let resp = handlers
                .list_credentials(&store, &actor)
                .await
                .map_err(app_service_error)?;
            let mut h = std::io::stdout().lock();
            for c in &resp.credentials {
                write_credential(&mut h, c)?;
            }
        }
        CredentialAction::Add {
            kind,
            name,
            value,
            description,
            provider,
            ..
        } => {
            let kind = parse_credential_kind(&kind)?;
            let resp = handlers
                .create_credential(
                    &store,
                    &actor,
                    CreateCredentialRequest {
                        kind,
                        name,
                        description,
                        provider,
                        value: SecretString::from(value),
                    },
                )
                .await
                .map_err(app_service_error)?;
            write_credential(&mut std::io::stdout().lock(), &resp.credential)?;
        }
        CredentialAction::Update {
            id,
            value,
            name,
            description,
            ..
        } => {
            let id = parse_credential_id(&id)?;
            let resp = handlers
                .update_credential(
                    &store,
                    &actor,
                    UpdateCredentialRequest {
                        id,
                        name,
                        description,
                        value: SecretString::from(value),
                    },
                )
                .await
                .map_err(app_service_error)?;
            write_credential(&mut std::io::stdout().lock(), &resp.credential)?;
        }
        CredentialAction::Remove { id, .. } => {
            let id = parse_credential_id(&id)?;
            let resp = handlers
                .remove_credential(&store, &actor, RemoveCredentialRequest { id })
                .await
                .map_err(app_service_error)?;
            writeln!(std::io::stdout().lock(), "removed={}", resp.removed)
                .context("write remove result")?;
        }
    }
    Ok(())
}

fn app_service_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Account(r) => anyhow::anyhow!("error: {} — {}", r.code(), r.summary()),
        AppServiceError::Configuration(r) => {
            anyhow::anyhow!("error: {} — {}", r.code(), r.summary())
        }
        AppServiceError::InvalidInput(m) => anyhow::anyhow!("error: validation_failed — {m}"),
        AppServiceError::Store(e) => anyhow::anyhow!("error: internal_error — {e}"),
        _ => anyhow::anyhow!("error: internal_error — unknown app-service failure"),
    }
}

async fn connect_and_authenticate(
    database_url: &str,
) -> Result<(Handlers, Store, AuthenticatedActor)> {
    let handlers = Handlers::new();
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let actor = resolve_actor(&handlers, &store).await?;
    Ok((handlers, store, actor))
}

async fn resolve_actor(handlers: &Handlers, store: &Store) -> Result<AuthenticatedActor> {
    let token = read_session()?;
    handlers
        .resolve_actor(store, &token)
        .await
        .map_err(app_service_error)
}

fn read_session() -> Result<SessionToken> {
    let path = session_path();
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read session from {}", path.display()))?;
    let trimmed = raw.trim().to_owned();
    if trimmed.is_empty() {
        anyhow::bail!("session file is empty — run `tanren-cli account sign-in` first");
    }
    Ok(SessionToken::from_secret(SecretString::from(trimmed)))
}

fn write_credential(h: &mut impl Write, c: &RedactedCredentialMetadata) -> Result<()> {
    writeln!(
        h,
        "id={} name={} kind={} scope={} present={}",
        c.id, c.name, c.kind, c.scope, c.present
    )
    .context("write credential")
}

fn parse_setting_key(raw: &str) -> Result<UserSettingKey> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned()))
        .with_context(|| format!("invalid setting key: {raw}"))
}

fn parse_credential_kind(raw: &str) -> Result<CredentialKind> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned()))
        .with_context(|| format!("invalid credential kind: {raw}"))
}

fn parse_credential_id(raw: &str) -> Result<CredentialId> {
    Ok(CredentialId::new(
        Uuid::parse_str(raw).context("parse credential id as UUID")?,
    ))
}
