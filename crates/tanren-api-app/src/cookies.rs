//! Cookie-session wiring: tower-sessions store dispatch (sqlite vs
//! postgres), the `SessionManagerLayer` builder, and the helper that
//! writes `(account_id, expires_at)` into a freshly minted session.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget.

use std::str::FromStr;

use anyhow::{Context, Result};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use tanren_app_services::AuthenticatedActor;
use tanren_identity_policy::AccountId;
use tower_sessions::cookie::SameSite;
use tower_sessions::cookie::time::Duration as CookieDuration;
use tower_sessions::{Expiry, Session, SessionManagerLayer};
use tower_sessions_sqlx_store::{PostgresStore, SqliteStore};

use crate::errors::AccountFailureBody;

const SESSION_COOKIE_NAME: &str = "tanren_session";
const SESSION_MAX_AGE_DAYS: i64 = 30;
const SESSION_KEY_ACCOUNT: &str = "account_id";
const SESSION_KEY_EXPIRES: &str = "expires_at";

/// `(account_id, expires_at)` projection of a freshly minted session.
/// All three account-flow handlers pass this into
/// [`install_cookie_session`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct SessionWrite {
    pub(crate) account_id: AccountId,
    pub(crate) expires_at: DateTime<Utc>,
}

/// Insert the account id and expiry into the tower-sessions row backing
/// this request. The cookie carrying the opaque session id is set by
/// the middleware on response — we just write the data.
pub(crate) async fn install_cookie_session(session: &Session, write: &SessionWrite) -> Result<()> {
    session
        .insert(SESSION_KEY_ACCOUNT, write.account_id)
        .await
        .context("insert account_id into session")?;
    session
        .insert(SESSION_KEY_EXPIRES, write.expires_at)
        .await
        .context("insert expires_at into session")?;
    Ok(())
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

pub(crate) async fn require_authenticated(
    session: &Session,
) -> Result<AuthenticatedActor, Response> {
    match session.get::<AccountId>(SESSION_KEY_ACCOUNT).await {
        Ok(Some(account_id)) => Ok(AuthenticatedActor::from_account_id(account_id)),
        Ok(None) => Err(unauthenticated()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read");
            Err(unauthenticated())
        }
    }
}

fn unauthenticated() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(AccountFailureBody {
            code: "unauthenticated".to_owned(),
            summary: "Authentication required.".to_owned(),
        }),
    )
        .into_response()
}
