//! Cookie-session wiring: tower-sessions store dispatch (sqlite vs
//! postgres), the `SessionManagerLayer` builder, and the helper that
//! writes `(account_id, expires_at)` into a freshly minted session.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget.

use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tanren_identity_policy::AccountId;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_sqlx_store::{PostgresStore, SqliteStore};

const SESSION_COOKIE_NAME: &str = "tanren_session";
const SESSION_MAX_AGE_DAYS: i64 = 30;
const SESSION_KEY_ACCOUNT: &str = "account_id";
const SESSION_KEY_EXPIRES: &str = "expires_at";
const SESSION_KEY_SIGNED_IN_ACCOUNT_IDS: &str = "signed_in_account_ids";
const SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW: &str = "active_account_by_window";
const SESSION_DEFAULT_WINDOW_KEY: &str = "_default";
const SESSION_SIGNED_IN_ACCOUNT_LIMIT: usize = 16;
const SESSION_WINDOW_MAP_LIMIT: usize = 16;

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
    window_key: Option<&str>,
) -> Result<()> {
    session
        .insert(SESSION_KEY_ACCOUNT, write.account_id)
        .await
        .context("insert account_id into session")?;
    session
        .insert(SESSION_KEY_EXPIRES, write.expires_at)
        .await
        .context("insert expires_at into session")?;

    let mut signed_in_ids = session
        .get::<Vec<AccountId>>(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS)
        .await
        .context("read signed_in_account_ids from session")?
        .unwrap_or_default();
    if !signed_in_ids.contains(&write.account_id) {
        signed_in_ids.push(write.account_id);
    }
    if signed_in_ids.len() > SESSION_SIGNED_IN_ACCOUNT_LIMIT {
        let drop_count = signed_in_ids.len() - SESSION_SIGNED_IN_ACCOUNT_LIMIT;
        signed_in_ids.drain(0..drop_count);
    }
    session
        .insert(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS, signed_in_ids)
        .await
        .context("insert signed_in_account_ids into session")?;

    let mut active_by_window = session
        .get::<BTreeMap<String, AccountId>>(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW)
        .await
        .context("read active_account_by_window from session")?
        .unwrap_or_default();
    active_by_window.insert(
        normalize_window_key(window_key).to_owned(),
        write.account_id,
    );
    while active_by_window.len() > SESSION_WINDOW_MAP_LIMIT {
        if let Some(oldest_key) = active_by_window.keys().next().cloned() {
            active_by_window.remove(&oldest_key);
        } else {
            break;
        }
    }
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
    window_key: Option<&str>,
) -> Result<Option<SessionAccountContext>> {
    let active_account = session
        .get::<AccountId>(SESSION_KEY_ACCOUNT)
        .await
        .context("read account_id from session")?;
    let mut signed_in_ids = session
        .get::<Vec<AccountId>>(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS)
        .await
        .context("read signed_in_account_ids from session")?
        .unwrap_or_default();

    if signed_in_ids.is_empty() {
        if let Some(account_id) = active_account {
            signed_in_ids.push(account_id);
        }
    }
    if signed_in_ids.is_empty() {
        return Ok(None);
    }

    let active_by_window = session
        .get::<BTreeMap<String, AccountId>>(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW)
        .await
        .context("read active_account_by_window from session")?
        .unwrap_or_default();

    let key = normalize_window_key(window_key);
    let active_for_window = active_by_window.get(key).copied();
    let mut active_account_id = active_for_window
        .or(active_account)
        .unwrap_or_else(|| signed_in_ids[0]);

    if !signed_in_ids.contains(&active_account_id) {
        active_account_id = signed_in_ids[0];
    }

    Ok(Some(SessionAccountContext {
        active_account_id,
        signed_in_account_ids: signed_in_ids,
    }))
}

/// Persist a successful active-account switch for one window scope.
pub(crate) async fn write_active_account_for_window(
    session: &Session,
    window_key: Option<&str>,
    active_account_id: AccountId,
) -> Result<()> {
    session
        .insert(SESSION_KEY_ACCOUNT, active_account_id)
        .await
        .context("insert account_id into session")?;

    let mut active_by_window = session
        .get::<BTreeMap<String, AccountId>>(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW)
        .await
        .context("read active_account_by_window from session")?
        .unwrap_or_default();
    active_by_window.insert(
        normalize_window_key(window_key).to_owned(),
        active_account_id,
    );
    while active_by_window.len() > SESSION_WINDOW_MAP_LIMIT {
        if let Some(oldest_key) = active_by_window.keys().next().cloned() {
            active_by_window.remove(&oldest_key);
        } else {
            break;
        }
    }
    session
        .insert(SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW, active_by_window)
        .await
        .context("insert active_account_by_window into session")?;
    Ok(())
}

fn normalize_window_key(window_key: Option<&str>) -> &str {
    window_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(SESSION_DEFAULT_WINDOW_KEY)
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
