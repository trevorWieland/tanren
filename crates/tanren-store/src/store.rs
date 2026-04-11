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

/// Backend-agnostic store handle. Cheap to clone — the inner
/// [`DatabaseConnection`] is itself a shared pool handle.
#[derive(Clone, Debug)]
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
