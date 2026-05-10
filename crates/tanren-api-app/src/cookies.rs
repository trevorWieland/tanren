//! Cookie-session wiring: tower-sessions store dispatch (sqlite vs
//! postgres), the `SessionManagerLayer` builder, and the helper that
//! writes `(account_id, expires_at)` into a freshly minted session.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget.

use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tanren_app_services::{ACTIVE_ACCOUNT_REGISTRY_LIMIT, ACTIVE_ACCOUNT_WINDOW_REGISTRY_LIMIT};
use tanren_contract::WindowContextId;
use tanren_identity_policy::AccountId;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_sqlx_store::{PostgresStore, SqliteStore};

const SESSION_COOKIE_NAME: &str = "tanren_session";
const SESSION_MAX_AGE_DAYS: i64 = 30;
const SESSION_KEY_ACCOUNT: &str = "account_id";
const SESSION_KEY_EXPIRES: &str = "expires_at";
const SESSION_KEY_SIGNED_IN_ACCOUNTS: &str = "signed_in_accounts";
// Legacy key retained for backward-compatible reads from older session rows.
const SESSION_KEY_SIGNED_IN_ACCOUNT_IDS: &str = "signed_in_account_ids";
const SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW: &str = "active_account_by_window";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedInAccountSessionEntry {
    account_id: AccountId,
    expires_at: DateTime<Utc>,
}

/// `(account_id, expires_at)` projection of a freshly minted session.
/// All three account-flow handlers pass this into
/// [`install_cookie_session`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionWrite {
    pub(crate) account_id: AccountId,
    pub(crate) expires_at: DateTime<Utc>,
}

/// Caller-bound active account state projected from the session row.
#[derive(Debug, Clone)]
pub(crate) struct SessionAccountContext {
    pub(crate) active_account_id: AccountId,
    pub(crate) signed_in_account_ids: Vec<AccountId>,
}

/// Insert the account id and expiry into the tower-sessions row backing
/// this request. The cookie carrying the opaque session id is set by
/// the middleware on response — we just write the data.
pub(crate) async fn install_cookie_session(
    session: &Session,
    write: &SessionWrite,
    window_context: WindowContextId,
) -> Result<()> {
    session
        .insert(SESSION_KEY_ACCOUNT, write.account_id)
        .await
        .context("insert account_id into session")?;
    session
        .insert(SESSION_KEY_EXPIRES, write.expires_at)
        .await
        .context("insert expires_at into session")?;

    let mut signed_in_accounts = read_signed_in_accounts(session).await?;
    signed_in_accounts.push(SignedInAccountSessionEntry {
        account_id: write.account_id,
        expires_at: write.expires_at,
    });
    let signed_in_accounts = sanitize_signed_in_accounts(signed_in_accounts, Utc::now());
    let signed_in_ids = signed_in_account_ids(&signed_in_accounts);
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNTS, signed_in_accounts.clone())
        .await
        .context("insert signed_in_accounts into session")?;
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS, signed_in_ids.clone())
        .await
        .context("insert signed_in_account_ids into session")?;

    let mut active_by_window = session
        .get::<BTreeMap<String, AccountId>>(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW)
        .await
        .context("read active_account_by_window from session")?
        .unwrap_or_default();
    sanitize_active_by_window_map(&mut active_by_window, &signed_in_ids);
    active_by_window.insert(normalize_window_key(window_context), write.account_id);
    trim_active_by_window_map(&mut active_by_window);
    session
        .insert(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW, active_by_window)
        .await
        .context("insert active_account_by_window into session")?;

    Ok(())
}

/// Resolve the active-account context from session state for one window
/// scope. Returns `Ok(None)` when no account session is present.
pub(crate) async fn read_session_account_context(
    session: &Session,
    window_context: WindowContextId,
) -> Result<Option<SessionAccountContext>> {
    let active_account = session
        .get::<AccountId>(SESSION_KEY_ACCOUNT)
        .await
        .context("read account_id from session")?;
    let mut signed_in_accounts = read_signed_in_accounts(session).await?;
    if signed_in_accounts.is_empty() {
        if let Some(account_id) = active_account {
            signed_in_accounts.push(SignedInAccountSessionEntry {
                account_id,
                expires_at: read_or_default_session_expiry(session).await?,
            });
        }
    }
    let signed_in_accounts = sanitize_signed_in_accounts(signed_in_accounts, Utc::now());
    let signed_in_ids = signed_in_account_ids(&signed_in_accounts);
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNTS, signed_in_accounts.clone())
        .await
        .context("insert signed_in_accounts into session")?;
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS, signed_in_ids.clone())
        .await
        .context("insert signed_in_account_ids into session")?;

    if signed_in_ids.is_empty() {
        session
            .insert(
                SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW,
                BTreeMap::<String, AccountId>::new(),
            )
            .await
            .context("clear active_account_by_window in session")?;
        return Ok(None);
    }

    let mut active_by_window = session
        .get::<BTreeMap<String, AccountId>>(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW)
        .await
        .context("read active_account_by_window from session")?
        .unwrap_or_default();
    sanitize_active_by_window_map(&mut active_by_window, &signed_in_ids);

    let key = normalize_window_key(window_context);
    let active_for_window = active_by_window.get(&key).copied();
    let mut active_account_id = active_for_window
        .or(active_account)
        .unwrap_or_else(|| signed_in_ids[0]);

    if !signed_in_ids.contains(&active_account_id) {
        active_account_id = signed_in_ids[0];
    }
    session
        .insert(SESSION_KEY_ACCOUNT, active_account_id)
        .await
        .context("insert account_id into session")?;
    let active_expires_at = expiry_for_account(&signed_in_accounts, active_account_id)
        .unwrap_or_else(default_session_expiry);
    session
        .insert(SESSION_KEY_EXPIRES, active_expires_at)
        .await
        .context("insert expires_at into session")?;
    session
        .insert(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW, active_by_window)
        .await
        .context("insert active_account_by_window into session")?;

    Ok(Some(SessionAccountContext {
        active_account_id,
        signed_in_account_ids: signed_in_ids,
    }))
}

/// Persist a successful active-account switch for one window scope.
pub(crate) async fn write_active_account_for_window(
    session: &Session,
    window_context: WindowContextId,
    active_account_id: AccountId,
) -> Result<()> {
    session
        .insert(SESSION_KEY_ACCOUNT, active_account_id)
        .await
        .context("insert account_id into session")?;

    let mut signed_in_accounts = read_signed_in_accounts(session).await?;
    let active_expires_at = expiry_for_account(&signed_in_accounts, active_account_id)
        .unwrap_or(read_or_default_session_expiry(session).await?);
    signed_in_accounts.push(SignedInAccountSessionEntry {
        account_id: active_account_id,
        expires_at: active_expires_at,
    });
    let signed_in_accounts = sanitize_signed_in_accounts(signed_in_accounts, Utc::now());
    let signed_in_ids = signed_in_account_ids(&signed_in_accounts);
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNTS, signed_in_accounts)
        .await
        .context("insert signed_in_accounts into session")?;
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS, signed_in_ids.clone())
        .await
        .context("insert signed_in_account_ids into session")?;

    let mut active_by_window = session
        .get::<BTreeMap<String, AccountId>>(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW)
        .await
        .context("read active_account_by_window from session")?
        .unwrap_or_default();
    sanitize_active_by_window_map(&mut active_by_window, &signed_in_ids);
    active_by_window.insert(normalize_window_key(window_context), active_account_id);
    trim_active_by_window_map(&mut active_by_window);
    session
        .insert(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW, active_by_window)
        .await
        .context("insert active_account_by_window into session")?;
    session
        .insert(SESSION_KEY_EXPIRES, active_expires_at)
        .await
        .context("insert expires_at into session")?;
    Ok(())
}

fn normalize_window_key(window_context: WindowContextId) -> String {
    window_context.to_string()
}

async fn read_signed_in_accounts(session: &Session) -> Result<Vec<SignedInAccountSessionEntry>> {
    let signed_in_accounts = session
        .get::<Vec<SignedInAccountSessionEntry>>(SESSION_KEY_SIGNED_IN_ACCOUNTS)
        .await
        .context("read signed_in_accounts from session")?
        .unwrap_or_default();
    if !signed_in_accounts.is_empty() {
        return Ok(signed_in_accounts);
    }

    let legacy_signed_in_ids = session
        .get::<Vec<AccountId>>(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS)
        .await
        .context("read signed_in_account_ids from session")?
        .unwrap_or_default();
    if legacy_signed_in_ids.is_empty() {
        return Ok(Vec::new());
    }
    let legacy_expires_at = read_or_default_session_expiry(session).await?;
    Ok(legacy_signed_in_ids
        .into_iter()
        .map(|account_id| SignedInAccountSessionEntry {
            account_id,
            expires_at: legacy_expires_at,
        })
        .collect())
}

async fn read_or_default_session_expiry(session: &Session) -> Result<DateTime<Utc>> {
    let expires_at = session
        .get::<DateTime<Utc>>(SESSION_KEY_EXPIRES)
        .await
        .context("read expires_at from session")?
        .unwrap_or_else(default_session_expiry);
    Ok(expires_at)
}

fn default_session_expiry() -> DateTime<Utc> {
    Utc::now() + ChronoDuration::days(SESSION_MAX_AGE_DAYS)
}

fn sanitize_signed_in_accounts(
    accounts: Vec<SignedInAccountSessionEntry>,
    now: DateTime<Utc>,
) -> Vec<SignedInAccountSessionEntry> {
    let mut deduped = Vec::with_capacity(accounts.len());
    let mut seen = HashSet::new();
    for entry in accounts.into_iter().rev() {
        if entry.expires_at <= now {
            continue;
        }
        if seen.insert(entry.account_id) {
            deduped.push(entry);
        }
    }
    deduped.reverse();
    if deduped.len() > ACTIVE_ACCOUNT_REGISTRY_LIMIT {
        let drop_count = deduped.len() - ACTIVE_ACCOUNT_REGISTRY_LIMIT;
        deduped.drain(0..drop_count);
    }
    deduped
}

fn signed_in_account_ids(accounts: &[SignedInAccountSessionEntry]) -> Vec<AccountId> {
    accounts.iter().map(|entry| entry.account_id).collect()
}

fn expiry_for_account(
    accounts: &[SignedInAccountSessionEntry],
    account_id: AccountId,
) -> Option<DateTime<Utc>> {
    accounts
        .iter()
        .find(|entry| entry.account_id == account_id)
        .map(|entry| entry.expires_at)
}

fn sanitize_active_by_window_map(
    active_by_window: &mut BTreeMap<String, AccountId>,
    signed_in_ids: &[AccountId],
) {
    let signed_in = signed_in_ids.iter().copied().collect::<HashSet<_>>();
    active_by_window.retain(|_, account_id| signed_in.contains(account_id));
    trim_active_by_window_map(active_by_window);
}

fn trim_active_by_window_map(active_by_window: &mut BTreeMap<String, AccountId>) {
    while active_by_window.len() > ACTIVE_ACCOUNT_WINDOW_REGISTRY_LIMIT {
        if let Some(oldest_key) = active_by_window.keys().next().cloned() {
            active_by_window.remove(&oldest_key);
        } else {
            break;
        }
    }
}

/// `tower-sessions` store wrapper. tower-sessions-sqlx-store ships
/// `SqliteStore` and `PostgresStore`; we dispatch on the URL scheme so
/// the same `serve` entry point covers both backends.
pub(crate) enum CookieStore {
    Sqlite(SqliteStore),
    Postgres(PostgresStore),
}

/// Build the appropriate `CookieStore` variant for the supplied
/// database URL and apply the tower-sessions migrations.
pub(crate) async fn build_cookie_store(database_url: &str) -> Result<CookieStore> {
    if database_url.starts_with("postgres:") || database_url.starts_with("postgresql:") {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("connect tower-sessions postgres pool")?;
        let store = PostgresStore::new(pool);
        store
            .migrate()
            .await
            .context("apply tower-sessions postgres migrations")?;
        Ok(CookieStore::Postgres(store))
    } else {
        let opts = sqlx::sqlite::SqliteConnectOptions::from_str(database_url)
            .context("parse sqlite connect options")?
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .context("connect tower-sessions sqlite pool")?;
        let store = SqliteStore::new(pool);
        store
            .migrate()
            .await
            .context("apply tower-sessions sqlite migrations")?;
        Ok(CookieStore::Sqlite(store))
    }
}

/// Hardened cookie-session layer. Profile contract:
/// `Secure + HttpOnly + SameSite=Strict + Path=/ + Max-Age=2592000`.
/// See `profiles/rust-cargo/architecture/cookie-session.md`.
pub(crate) fn session_layer(store: CookieStore) -> SessionLayerEnum {
    session_layer_with_secure(store, true)
}

/// Build the cookie-session layer with an explicit `secure` flag. The BDD
/// wire-harness drives the API over plain HTTP on an ephemeral port and
/// must disable the `Secure` attribute so cookies survive the loopback
/// hop; production callers always use [`session_layer`] (secure = true).
pub(crate) fn session_layer_with_secure(store: CookieStore, secure: bool) -> SessionLayerEnum {
    let expiry = Expiry::OnInactivity(CookieDuration::days(SESSION_MAX_AGE_DAYS));
    match store {
        CookieStore::Sqlite(s) => SessionLayerEnum::Sqlite(
            SessionManagerLayer::new(s)
                .with_name(SESSION_COOKIE_NAME)
                .with_secure(secure)
                .with_http_only(true)
                .with_same_site(SameSite::Strict)
                .with_path("/")
                .with_expiry(expiry),
        ),
        CookieStore::Postgres(s) => SessionLayerEnum::Postgres(
            SessionManagerLayer::new(s)
                .with_name(SESSION_COOKIE_NAME)
                .with_secure(secure)
                .with_http_only(true)
                .with_same_site(SameSite::Strict)
                .with_path("/")
                .with_expiry(expiry),
        ),
    }
}

pub(crate) enum SessionLayerEnum {
    Sqlite(SessionManagerLayer<SqliteStore>),
    Postgres(SessionManagerLayer<PostgresStore>),
}
