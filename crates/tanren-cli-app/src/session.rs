use std::collections::{BTreeMap, HashSet};
use std::env;
use std::error::Error as StdError;

use anyhow::Result;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::{
    AccountErrorProjection, AccountStore, ActiveAccountContext, ActiveAccountContextError, Clock,
};
use tanren_client_integrations::session_file_store::SessionFileStore;
use tanren_identity_policy::{AccountId, SessionToken};

const WINDOW_ID_ENV: &str = "TANREN_WINDOW_ID";
const DEFAULT_WINDOW_KEY: &str = "_default";
const SESSION_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CliSessionFile {
    version: u32,
    #[serde(default)]
    signed_in: Vec<SignedInSession>,
    #[serde(default)]
    active_account_by_window: BTreeMap<String, AccountId>,
}

impl Default for CliSessionFile {
    fn default() -> Self {
        Self {
            version: SESSION_FILE_VERSION,
            signed_in: Vec::new(),
            active_account_by_window: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedInSession {
    account_id: AccountId,
    token: SessionToken,
}

pub(crate) async fn active_context_from_session<S>(
    store: &S,
    clock: &Clock,
) -> Result<ActiveAccountContext>
where
    S: AccountStore + ?Sized,
{
    let mut session = read_session_file()?;
    let mut signed_in = Vec::with_capacity(session.signed_in.len());
    let mut seen_account_ids = HashSet::new();
    let now = clock.now();
    let mut mutated = false;

    for entry in &session.signed_in {
        match store.validate_session_token(&entry.token, now).await {
            Ok(account) => {
                if account.id != entry.account_id {
                    mutated = true;
                }
                if seen_account_ids.insert(account.id) {
                    signed_in.push(SignedInSession {
                        account_id: account.id,
                        token: entry.token.clone(),
                    });
                } else {
                    mutated = true;
                }
            }
            Err(err) => {
                if StdError::source(&err).is_some() {
                    tracing::error!(target: "tanren_cli", error = %err, "session token validation");
                    let projected = AccountErrorProjection::internal();
                    return Err(anyhow::anyhow!(
                        "error: {} — {}",
                        projected.code,
                        projected.summary
                    ));
                }
                mutated = true;
            }
        }
    }

    if session.signed_in.len() != signed_in.len() {
        mutated = true;
    }
    session.signed_in = signed_in;

    let before_window_count = session.active_account_by_window.len();
    session
        .active_account_by_window
        .retain(|_, id| seen_account_ids.contains(id));
    if before_window_count != session.active_account_by_window.len() {
        mutated = true;
    }

    if session.signed_in.is_empty() {
        if !session.active_account_by_window.is_empty() {
            session.active_account_by_window.clear();
            mutated = true;
        }
        if mutated {
            write_session_file(&session)?;
        }
        return Err(anyhow::anyhow!(
            "error: invalid_credential — no signed-in accounts are available in the CLI session file"
        ));
    }

    let signed_in_account_ids = session
        .signed_in
        .iter()
        .map(|entry| entry.account_id)
        .collect::<Vec<_>>();
    let key = window_key();
    let active_account_id = session
        .active_account_by_window
        .get(&key)
        .copied()
        .filter(|id| signed_in_account_ids.contains(id))
        .unwrap_or(signed_in_account_ids[0]);
    if session.active_account_by_window.get(&key).copied() != Some(active_account_id) {
        session
            .active_account_by_window
            .insert(key, active_account_id);
        mutated = true;
    }
    if mutated {
        write_session_file(&session)?;
    }
    ActiveAccountContext::from_account_ids(active_account_id, signed_in_account_ids)
        .map_err(|err| map_active_account_context_error(&err))
}

pub(crate) fn persist_session(account_id: AccountId, token: &str) -> Result<()> {
    let mut session = read_session_file()?;
    if let Some(existing) = session
        .signed_in
        .iter_mut()
        .find(|entry| entry.account_id == account_id)
    {
        existing.token = SessionToken::from_secret(SecretString::from(token.to_owned()));
    } else {
        session.signed_in.push(SignedInSession {
            account_id,
            token: SessionToken::from_secret(SecretString::from(token.to_owned())),
        });
    }
    session
        .active_account_by_window
        .insert(window_key(), account_id);
    write_session_file(&session)
}

pub(crate) fn set_active_account(account_id: AccountId) -> Result<()> {
    let mut session = read_session_file()?;
    if !session
        .signed_in
        .iter()
        .any(|entry| entry.account_id == account_id)
    {
        return Err(anyhow::anyhow!(
            "error: target_account_not_signed_in — requested account is not present in this session file"
        ));
    }
    session
        .active_account_by_window
        .insert(window_key(), account_id);
    write_session_file(&session)
}

fn window_key() -> String {
    env::var(WINDOW_ID_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_WINDOW_KEY.to_owned())
}

fn read_session_file() -> Result<CliSessionFile> {
    session_store()
        .read_json_or_default(|_| Some(CliSessionFile::default()))
        .map_err(anyhow::Error::from)
}

fn write_session_file(session: &CliSessionFile) -> Result<()> {
    session_store()
        .write_json_pretty(session)
        .map_err(anyhow::Error::from)
}

fn session_store() -> SessionFileStore {
    SessionFileStore::from_env()
}

fn map_active_account_context_error(err: &ActiveAccountContextError) -> anyhow::Error {
    anyhow::anyhow!("error: validation_failed — {err}")
}
