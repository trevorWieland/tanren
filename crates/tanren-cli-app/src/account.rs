use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_identity_policy::{Email, InvitationToken};

#[derive(Debug, Subcommand)]
pub(crate) enum AccountAction {
    Create {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        identifier: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value_t = String::from("Tanren user"))]
        display_name: String,
        #[arg(long)]
        invitation: Option<String>,
    },
    SignIn {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        identifier: String,
        #[arg(long)]
        password: String,
    },
}

pub(crate) fn dispatch_account(action: AccountAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_account(action))
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
                    let response = handlers
                        .sign_up(
                            &store,
                            SignUpRequest {
                                email,
                                password,
                                display_name,
                            },
                        )
                        .await
                        .map_err(account_error)?;
                    persist_session(response.session.token.expose_secret())?;
                    let stdout = std::io::stdout();
                    let mut handle = stdout.lock();
                    writeln!(
                        handle,
                        "account_id={id} session={token}",
                        id = response.account.id,
                        token = response.session.token.expose_secret(),
                    )
                    .context("write sign-up result")?;
                }
                Some(token) => {
                    let invitation_token = InvitationToken::parse(&token)
                        .context("parse --invitation as invitation token")?;
                    let response = handlers
                        .accept_invitation(
                            &store,
                            AcceptInvitationRequest {
                                invitation_token,
                                email,
                                password,
                                display_name,
                            },
                        )
                        .await
                        .map_err(account_error)?;
                    persist_session(response.session.token.expose_secret())?;
                    let stdout = std::io::stdout();
                    let mut handle = stdout.lock();
                    writeln!(
                        handle,
                        "account_id={id} session={token} joined_org={org}",
                        id = response.account.id,
                        token = response.session.token.expose_secret(),
                        org = response.joined_org,
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
            let response = handlers
                .sign_in(&store, SignInRequest { email, password })
                .await
                .map_err(account_error)?;
            persist_session(response.session.token.expose_secret())?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "account_id={id} session={token}",
                id = response.account.id,
                token = response.session.token.expose_secret(),
            )
            .context("write sign-in result")?;
        }
    }
    Ok(())
}

fn account_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Account(reason) => {
            anyhow::anyhow!("error: {} — {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(message) => {
            anyhow::anyhow!("error: validation_failed — {message}")
        }
        AppServiceError::Store(err) => {
            anyhow::anyhow!("error: internal_error — {err}")
        }
        _ => anyhow::anyhow!("error: internal_error — unknown app-service failure"),
    }
}

fn session_path() -> PathBuf {
    if let Ok(explicit) = env::var("TANREN_SESSION_FILE") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    let base = env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map_or_else(
            || {
                env::var("HOME").ok().map_or_else(
                    || PathBuf::from("."),
                    |home| PathBuf::from(home).join(".local/state"),
                )
            },
            PathBuf::from,
        );
    base.join("tanren").join("session")
}

fn persist_session(token: &str) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    fs::write(&path, token).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}
