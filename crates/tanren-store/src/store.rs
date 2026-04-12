//! Unified [`Store`] struct — one [`DatabaseConnection`] threaded
//! through every trait impl.
//!
//! The three traits ([`EventStore`](crate::EventStore),
//! [`JobQueue`](crate::JobQueue),
//! [`StateStore`](crate::StateStore)) are implemented for `Store` in
//! their respective modules. This file only holds the struct
//! definition, the constructor, the migration runner, and a
//! crate-internal accessor for the underlying connection.

use sea_orm::{DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

use crate::connection;
use crate::errors::{StoreError, StoreResult};
use crate::migration::Migrator;

/// Backend-agnostic store handle.
///
/// Wrap in [`std::sync::Arc`] if you need to share it across tasks.
/// We deliberately do not derive [`Clone`] because `SeaORM`'s
/// [`DatabaseConnection`] is not `Clone` when the `mock` feature is
/// active (which the store enables in dev-dependencies for unit
/// tests). Since Lane 0.4 will share `Store` via `Arc` anyway, the
/// lost convenience is minimal.
#[derive(Debug)]
pub struct Store {
    conn: DatabaseConnection,
}

impl Store {
    /// Open a connection to the given database URL. Does **not** run
    /// migrations — call [`Store::run_migrations`] separately.
    ///
    /// # Errors
    ///
    /// Returns any connection error raised by `SeaORM` (bad URL,
    /// unreachable host, authentication failure).
    pub async fn new(database_url: &str) -> StoreResult<Self> {
        let conn = connection::connect(database_url).await?;
        Ok(Self { conn })
    }

    /// Wrap an existing [`DatabaseConnection`]. Useful for tests that
    /// already hold a mock or in-memory connection.
    #[must_use]
    pub fn from_connection(conn: DatabaseConnection) -> Self {
        Self { conn }
    }

    /// Apply every pending schema migration.
    ///
    /// `SeaORM` tracks applied migrations in the `seaql_migrations`
    /// table, so this is idempotent — calling it twice in a row is a
    /// no-op on the second call.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Migration`] if the migrator rejects the
    /// schema (e.g., partially-applied state from an aborted run).
    pub async fn run_migrations(&self) -> StoreResult<()> {
        Migrator::up(&self.conn, None)
            .await
            .map_err(|err: DbErr| StoreError::Migration(err.to_string()))
    }

    /// Close the underlying connection pool.
    ///
    /// # Errors
    ///
    /// Returns any driver error raised during shutdown.
    pub async fn close(self) -> StoreResult<()> {
        self.conn.close().await.map_err(StoreError::from)
    }

    /// Accessor for the underlying connection. Used internally by the
    /// trait implementations.
    pub(crate) fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, DbBackend, MockDatabase};

    use super::*;

    #[test]
    fn from_connection_wraps_an_existing_handle() {
        let conn = MockDatabase::new(DbBackend::Postgres).into_connection();
        let store = Store::from_connection(conn);
        // `conn` is a valid handle — the crate-internal accessor
        // returns a reference we can inspect.
        assert_eq!(
            store.conn().get_database_backend(),
            DbBackend::Postgres,
            "Store::from_connection must preserve the underlying backend"
        );
    }

    #[test]
    fn from_connection_preserves_sqlite_backend() {
        let conn = MockDatabase::new(DbBackend::Sqlite).into_connection();
        let store = Store::from_connection(conn);
        assert_eq!(store.conn().get_database_backend(), DbBackend::Sqlite);
    }

    #[tokio::test]
    async fn new_rejects_obviously_invalid_url() {
        // Not a `sqlite:` or `postgres:` scheme — `SeaORM` rejects
        // the URL in its URL parser, returning a `DbErr`. The error
        // surfaces as `StoreError::Database`.
        let result = Store::new("gopher://nowhere").await;
        assert!(matches!(result, Err(StoreError::Database(_))));
    }
}
