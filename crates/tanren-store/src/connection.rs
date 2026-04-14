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

/// Pool sizing and timeout knobs for the database connection.
///
/// Every field corresponds to a `SeaORM` [`ConnectOptions`] setter.
/// The defaults match the hardcoded values the store used prior to
/// this struct's introduction, so callers that do not need custom
/// tuning can use [`ConnectConfig::default()`].
#[derive(Debug, Clone)]
pub struct ConnectConfig {
    /// Minimum number of connections the pool keeps open.
    pub min_connections: u32,
    /// Maximum number of connections the pool may open.
    pub max_connections: u32,
    /// Timeout for establishing a new connection.
    pub connect_timeout: Duration,
    /// How long an idle connection sits in the pool before being
    /// closed.
    pub idle_timeout: Duration,
}

impl Default for ConnectConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 8,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

/// Open a connection pool to the given database URL using the
/// default pool configuration.
///
/// # Errors
///
/// Returns any [`DbErr`] raised by `SeaORM` during connection — bad
/// URL, unreachable host, failed handshake, etc.
pub(crate) async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    connect_with_config(url, &ConnectConfig::default()).await
}

/// Open a connection pool to the given database URL using the
/// supplied pool configuration.
///
/// # Errors
///
/// Returns any [`DbErr`] raised by `SeaORM` during connection — bad
/// URL, unreachable host, failed handshake, etc.
pub(crate) async fn connect_with_config(
    url: &str,
    config: &ConnectConfig,
) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(url.to_owned());
    opt.min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .connect_timeout(config.connect_timeout)
        .idle_timeout(config.idle_timeout)
        .sqlx_logging(false);
    // `sqlx-sqlite` does not accept `busy_timeout` as a URL query
    // parameter, so we set it programmatically via SeaORM's sqlx
    // options hook. Called once per pool connection; non-SQLite
    // backends ignore this hook entirely.
    opt.map_sqlx_sqlite_opts(|options| {
        options.busy_timeout(SQLITE_BUSY_TIMEOUT).foreign_keys(true)
    });
    Database::connect(opt).await
}
