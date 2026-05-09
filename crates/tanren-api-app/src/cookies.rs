//! Cookie-session wiring: tower-sessions store dispatch (sqlite vs
//! postgres), the `SessionManagerLayer` builder, and the helper that
//! writes `(account_id, expires_at)` into a freshly minted session.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget.

use std::collections::{BTreeMap, HashSet};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tanren_app_services::{ACTIVE_ACCOUNT_REGISTRY_LIMIT, ACTIVE_ACCOUNT_WINDOW_REGISTRY_LIMIT};
use tanren_identity_policy::AccountId;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_sqlx_store::{PostgresStore, SqliteStore};

use crate::window_context::WindowContextId;

const SESSION_COOKIE_NAME: &str = "tanren_session";
const SESSION_MAX_AGE_DAYS: i64 = 30;
const SESSION_KEY_ACCOUNT: &str = "account_id";
const SESSION_KEY_EXPIRES: &str = "expires_at";
const SESSION_KEY_SIGNED_IN_ACCOUNT_IDS: &str = "signed_in_account_ids";
const SESSION_KEY_ACTIVE_ACCOUNT_BY_WINDOW: &str = "active_account_by_window";
const SESSION_DEFAULT_WINDOW_KEY: &str = "_default";

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
    window_context: Option<WindowContextId>,
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
    signed_in_ids.push(write.account_id);
    let signed_in_ids = sanitize_signed_in_account_ids(signed_in_ids);
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
    window_context: Option<WindowContextId>,
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
    let signed_in_ids = sanitize_signed_in_account_ids(signed_in_ids);
    if signed_in_ids.is_empty() {
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

    Ok(Some(SessionAccountContext {
        active_account_id,
        signed_in_account_ids: signed_in_ids,
    }))
}

/// Persist a successful active-account switch for one window scope.
pub(crate) async fn write_active_account_for_window(
    session: &Session,
    window_context: Option<WindowContextId>,
    active_account_id: AccountId,
) -> Result<()> {
    session
        .insert(SESSION_KEY_ACCOUNT, active_account_id)
        .await
        .context("insert account_id into session")?;

    let mut signed_in_ids = session
        .get::<Vec<AccountId>>(SESSION_KEY_SIGNED_IN_ACCOUNT_IDS)
        .await
        .context("read signed_in_account_ids from session")?
        .unwrap_or_default();
    signed_in_ids.push(active_account_id);
    let signed_in_ids = sanitize_signed_in_account_ids(signed_in_ids);
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
    Ok(())
}

fn normalize_window_key(window_context: Option<WindowContextId>) -> String {
    window_context.map_or_else(
        || SESSION_DEFAULT_WINDOW_KEY.to_owned(),
        WindowContextId::as_session_key,
    )
}

fn sanitize_signed_in_account_ids(ids: Vec<AccountId>) -> Vec<AccountId> {
    let mut deduped = Vec::with_capacity(ids.len());
    let mut seen = HashSet::new();
    for account_id in ids {
        if seen.insert(account_id) {
            deduped.push(account_id);
        }
    }
    if deduped.len() > ACTIVE_ACCOUNT_REGISTRY_LIMIT {
        let drop_count = deduped.len() - ACTIVE_ACCOUNT_REGISTRY_LIMIT;
        deduped.drain(0..drop_count);
    }
    deduped
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
