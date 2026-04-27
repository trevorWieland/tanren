//! Unified [`Store`] struct — one [`DatabaseConnection`] threaded
//! through every trait impl.
//!
//! The three traits ([`EventStore`](crate::EventStore),
//! [`JobQueue`](crate::JobQueue),
//! [`StateStore`](crate::StateStore)) are implemented for `Store` in
//! their respective modules. This file only holds the struct
//! definition, the constructor, the migration runner, and a
//! crate-internal accessor for the underlying connection.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use sea_orm_migration::MigratorTrait;

use crate::connection::{self, ConnectConfig};
use crate::db_error_codes::{extract_db_error_code, is_postgres_undefined_table_code};
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

    /// Open a connection with explicit pool sizing and timeout knobs.
    ///
    /// # Errors
    ///
    /// Returns any connection error raised by `SeaORM`.
    pub async fn new_with_config(database_url: &str, config: &ConnectConfig) -> StoreResult<Self> {
        let conn = connection::connect_with_config(database_url, config).await?;
        Ok(Self { conn })
    }

    /// Open a connection **and** apply all pending migrations.
    ///
    /// This is the recommended entrypoint for production use.
    /// Equivalent to calling [`Store::new`] followed by
    /// [`Store::run_migrations`].
    ///
    /// # Errors
    ///
    /// Returns connection errors or migration failures.
    pub async fn open_and_migrate(database_url: &str) -> StoreResult<Self> {
        let store = Self::new(database_url).await?;
        store.run_migrations().await?;
        Ok(store)
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

    /// Verify schema readiness for read-only operations.
    ///
    /// Read commands should not perform implicit schema writes. This
    /// preflight enforces that the database has already been migrated
    /// to the latest expected revision.
    pub async fn assert_schema_ready(&self) -> StoreResult<()> {
        let backend = self.conn.get_database_backend();
        if backend == DbBackend::Sqlite && !self.sqlite_has_migration_metadata_table().await? {
            return Err(StoreError::SchemaNotReady {
                reason: "missing migration metadata table".to_owned(),
            });
        }
        let rows = self
            .conn
            .query_all(Statement::from_string(
                backend,
                "SELECT version FROM seaql_migrations ORDER BY version DESC LIMIT 1",
            ))
            .await
            .map_err(|err| {
                if backend == DbBackend::Postgres
                    && extract_db_error_code(&err)
                        .as_deref()
                        .is_some_and(is_postgres_undefined_table_code)
                {
                    StoreError::SchemaNotReady {
                        reason: "missing migration metadata table".to_owned(),
                    }
                } else {
                    StoreError::from(err)
                }
            })?;

        let current = rows
            .first()
            .and_then(|row| row.try_get::<String>("", "version").ok())
            .ok_or_else(|| StoreError::SchemaNotReady {
                reason: "no applied migrations recorded".to_owned(),
            })?;

        if current != Migrator::LATEST_MIGRATION_NAME {
            return Err(StoreError::SchemaNotReady {
                reason: format!(
                    "expected latest migration `{}`, found `{current}`",
                    Migrator::LATEST_MIGRATION_NAME
                ),
            });
        }
        Ok(())
    }

    async fn sqlite_has_migration_metadata_table(&self) -> StoreResult<bool> {
        let rows = self
            .conn
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'seaql_migrations' LIMIT 1",
            ))
            .await?;
        Ok(!rows.is_empty())
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
