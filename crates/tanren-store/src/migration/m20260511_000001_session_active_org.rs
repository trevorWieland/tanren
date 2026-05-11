//! R-0004 migration: add `active_org_id` to `account_sessions`.
//!
//! The active-org column stores the organization currently in focus for
//! each session. `NULL` means the session has no active organization
//! (personal-account mode). A foreign key to `organizations(id)` ensures
//! referential integrity; `ON DELETE SET NULL` clears the active org
//! when an organization is deleted rather than blocking the deletion.
//!
//! `SQLite` does not support adding foreign key constraints to existing
//! tables via `ALTER TABLE`, so the FK is only created on Postgres.
//! The column itself works on both backends.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AccountSessions::Table)
                    .add_column(ColumnDef::new(AccountSessions::ActiveOrgId).uuid())
                    .to_owned(),
            )
            .await?;

        // `SQLite` does not support adding FK constraints to existing tables.
        let backend = manager.get_database_backend();
        if matches!(backend, DatabaseBackend::Postgres) {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_account_sessions_active_org")
                        .from(AccountSessions::Table, AccountSessions::ActiveOrgId)
                        .to(Organizations::Table, Organizations::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .on_update(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if matches!(backend, DatabaseBackend::Postgres) {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_account_sessions_active_org")
                        .table(AccountSessions::Table)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(AccountSessions::Table)
                    .drop_column(AccountSessions::ActiveOrgId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AccountSessions {
    Table,
    ActiveOrgId,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}
