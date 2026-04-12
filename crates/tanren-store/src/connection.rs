//! Database connection factory.
//!
//! The store threads a single [`DatabaseConnection`] through every
//! trait implementation. `SeaORM`'s connection type is internally a
//! reference-counted pool for network backends and a single handle
//! for `SQLite`, so sharing one instance across [`EventStore`],
//! [`JobQueue`], and [`StateStore`] is cheap and correct.
//!
//! Callers pass a URL. `sqlite://` and `postgres://` schemes are
//! supported; anything else is rejected by `SeaORM` at connect time.
//!
//! For `SQLite`, the connector enforces `busy_timeout = 5 seconds`
//! via `SeaORM`'s `map_sqlx_sqlite_opts` hook. This makes the
//! atomic dequeue path race-safe across processes by pushing
//! contention retries into the driver instead of bubbling
//! `SQLITE_BUSY` up to the caller. The value is set programmatically
//! (not via URL query parameter, which `sqlx-sqlite` does not accept
//! for `busy_timeout`).
//!
//! [`EventStore`]: crate::EventStore
//! [`JobQueue`]: crate::JobQueue
//! [`StateStore`]: crate::StateStore

use std::time::Duration;

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

/// How long the `SQLite` driver retries on a busy database before
/// surfacing `SQLITE_BUSY`. Five seconds is long enough for normal
/// contention between dequeue workers on a shared database but
/// short enough to surface genuine deadlocks.
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Open a connection pool to the given database URL.
///
/// # Errors
///
/// Returns any [`DbErr`] raised by `SeaORM` during connection — bad
/// URL, unreachable host, failed handshake, etc.
pub(crate) async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(url.to_owned());
    opt.min_connections(1)
        .max_connections(8)
        .connect_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(60))
        .sqlx_logging(false);
    // `sqlx-sqlite` does not accept `busy_timeout` as a URL query
    // parameter, so we set it programmatically via SeaORM's sqlx
    // options hook. Called once per pool connection; non-SQLite
    // backends ignore this hook entirely.
    opt.map_sqlx_sqlite_opts(|options| options.busy_timeout(SQLITE_BUSY_TIMEOUT));
    Database::connect(opt).await
}
