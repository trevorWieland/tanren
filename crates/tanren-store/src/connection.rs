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
//! For `SQLite`, include `?mode=rwc` to create the file if missing,
//! and append `?busy_timeout=5000` (ms) to get automatic retries on
//! `SQLITE_BUSY` so the atomic dequeue path does not propagate
//! spurious contention errors.
//!
//! [`EventStore`]: crate::EventStore
//! [`JobQueue`]: crate::JobQueue
//! [`StateStore`]: crate::StateStore

use std::time::Duration;

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

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
    Database::connect(opt).await
}
