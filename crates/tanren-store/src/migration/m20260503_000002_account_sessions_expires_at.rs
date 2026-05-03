//! R-0001 sub-PR 4 migration: add `expires_at` to `account_sessions` and a
//! partial-unique index on `invitations(token) WHERE consumed_at IS NULL`.
//!
//! `expires_at` is required for cookie/session expiry policy and the M5
//! finding from the R-0001 audit. The partial-unique index belt-and-braces
//! the atomic `consume_invitation` path: even if two callers race the
//! `UPDATE ... WHERE consumed_at IS NULL` filter on the same token, the
//! database rejects all but one of them.
//!
//! `MySQL` does not support partial indexes, so the index step is a no-op
//! there and the application-level `WHERE consumed_at IS NULL` filter is
//! the sole guarantee.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add `expires_at` to `account_sessions`. Existing rows (none in
        // production at R-0001 time) are backfilled with `current_timestamp`
        // so the column can be NOT NULL; the application threads the real
        // value (`now + 30 days`) on every insert going forward.
        manager
            .alter_table(
                Table::alter()
                    .table(AccountSessions::Table)
                    .add_column(
                        ColumnDef::new(AccountSessions::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Partial unique index on `invitations(token) WHERE consumed_at IS
        // NULL`. SeaORM's IndexCreateStatement has no built-in partial-WHERE
        // builder, so we issue raw SQL on the backends that support it.
        let backend = manager.get_database_backend();
        match backend {
            DatabaseBackend::Sqlite | DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "CREATE UNIQUE INDEX IF NOT EXISTS idx_invitations_active_token \
                         ON invitations (token) WHERE consumed_at IS NULL",
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                // MySQL has no partial indexes; the application-level
                // `WHERE consumed_at IS NULL` filter on the atomic UPDATE is
                // the sole guarantee on that backend.
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if matches!(backend, DatabaseBackend::Sqlite | DatabaseBackend::Postgres) {
            manager
                .get_connection()
                .execute_unprepared("DROP INDEX IF EXISTS idx_invitations_active_token")
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(AccountSessions::Table)
                    .drop_column(AccountSessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum AccountSessions {
    Table,
    ExpiresAt,
}
