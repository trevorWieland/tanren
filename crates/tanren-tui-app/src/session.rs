//! Session-file state for bearer-mode TUI account switching.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tanren_app_services::ActiveAccountContext;
use tanren_identity_policy::AccountId;

const SESSION_FILE_ENV: &str = "TANREN_SESSION_FILE";
const WINDOW_ID_ENV: &str = "TANREN_WINDOW_ID";
const DEFAULT_WINDOW_KEY: &str = "_default";
const SESSION_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TuiSessionFile {
    version: u32,
    #[serde(default)]
    signed_in: Vec<SignedInSession>,
    #[serde(default)]
    active_account_by_window: BTreeMap<String, AccountId>,
}

impl Default for TuiSessionFile {
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
    token: String,
}

pub(crate) fn persist_session(account_id: AccountId, token: &str) -> Result<()> {
    let mut session = read_session_file()?;
    if let Some(existing) = session
        .signed_in
        .iter_mut()
        .find(|entry| entry.account_id == account_id)
    {
        token.clone_into(&mut existing.token);
    } else {
        session.signed_in.push(SignedInSession {
            account_id,
            token: token.to_owned(),
        });
    }
    session
        .active_account_by_window
        .insert(window_key(), account_id);
    write_session_file(&session)
}

pub(crate) fn active_context_from_session() -> Result<ActiveAccountContext> {
    let session = read_session_file()?;
    if session.signed_in.is_empty() {
        return Err(anyhow::anyhow!(
            "invalid_credential: no signed-in account in session file"
        ));
    }
    let signed_in_account_ids = session
        .signed_in
        .iter()
        .map(|entry| entry.account_id)
        .collect::<Vec<_>>();
    let active_account_id = session
        .active_account_by_window
        .get(&window_key())
        .copied()
        .filter(|id| signed_in_account_ids.contains(id))
        .unwrap_or(signed_in_account_ids[0]);
    Ok(ActiveAccountContext::from_account_ids(
        active_account_id,
        signed_in_account_ids,
    ))
}

pub(crate) fn set_active_account(account_id: AccountId) -> Result<()> {
    let mut session = read_session_file()?;
    if !session
        .signed_in
        .iter()
        .any(|entry| entry.account_id == account_id)
    {
        return Err(anyhow::anyhow!(
            "target_account_not_signed_in: account missing from session file"
        ));
    }
    session
        .active_account_by_window
        .insert(window_key(), account_id);
    write_session_file(&session)
}

fn session_path() -> PathBuf {
    if let Ok(explicit) = env::var(SESSION_FILE_ENV) {
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

fn window_key() -> String {
    env::var(WINDOW_ID_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_WINDOW_KEY.to_owned())
}

fn read_session_file() -> Result<TuiSessionFile> {
    let path = session_path();
    if !path.exists() {
        return Ok(TuiSessionFile::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read session file from {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(TuiSessionFile::default());
    }
    if raw.trim_start().starts_with('{') {
        return serde_json::from_str::<TuiSessionFile>(&raw)
            .with_context(|| format!("parse session file {}", path.display()));
    }
    Ok(TuiSessionFile::default())
}

fn write_session_file(session: &TuiSessionFile) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(session).context("encode session file JSON")?;
    fs::write(&path, body).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}
