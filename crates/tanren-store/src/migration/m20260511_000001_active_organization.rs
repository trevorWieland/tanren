//! R-0004 migration: add nullable `active_org_id` to `account_sessions`
//! and create the minimal `projects` read-model fixture table.
//!
//! `active_org_id` is nullable — `NULL` means no active organization
//! (personal account or not yet switched). FK and index on the column
//! enable efficient session→active-org resolution.
//!
//! The `projects` table is the smallest read-model fixture needed to
//! witness B-0047 (project listing scoped to the active organization).
//! Full project lifecycle (create/update/delete) is out of scope.

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
        add_active_org_id_to_sessions(manager).await?;
        create_sessions_active_org_index(manager).await?;
        create_projects_table(manager).await?;
        create_projects_indexes(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_projects_indexes(manager).await?;
        drop_projects_table(manager).await?;
        drop_sessions_active_org_index(manager).await?;
        drop_active_org_id_from_sessions(manager).await?;
        Ok(())
    }
}

async fn add_active_org_id_to_sessions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(AccountSessions::Table)
                .add_column(ColumnDef::new(AccountSessions::ActiveOrgId).uuid())
                .to_owned(),
        )
        .await?;

    // SQLite does not support adding FK constraints to existing tables.
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

async fn create_sessions_active_org_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_account_sessions_account_active_org")
                .table(AccountSessions::Table)
                .col(AccountSessions::AccountId)
                .col(AccountSessions::ActiveOrgId)
                .to_owned(),
        )
        .await
}

async fn create_projects_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut fk_org = ForeignKey::create();
    fk_org
        .name("fk_projects_org")
        .from(Projects::Table, Projects::OrgId)
        .to(Organizations::Table, Organizations::Id)
        .on_delete(ForeignKeyAction::Cascade)
        .on_update(ForeignKeyAction::Cascade);

    manager
        .create_table(
            Table::create()
                .table(Projects::Table)
                .if_not_exists()
                .col(ColumnDef::new(Projects::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Projects::OrgId).uuid().not_null())
                .col(ColumnDef::new(Projects::Name).string().not_null())
                .col(
                    ColumnDef::new(Projects::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(&mut fk_org)
                .to_owned(),
        )
        .await
}

async fn create_projects_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_projects_org_id")
                .table(Projects::Table)
                .col(Projects::OrgId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_projects_org_name_unique")
                .table(Projects::Table)
                .col(Projects::OrgId)
                .col(Projects::Name)
                .unique()
                .to_owned(),
        )
        .await
}

async fn drop_projects_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_projects_org_name_unique")
                .table(Projects::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_index(
            Index::drop()
                .name("idx_projects_org_id")
                .table(Projects::Table)
                .to_owned(),
        )
        .await
}

async fn drop_projects_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(Projects::Table).to_owned())
        .await
}

async fn drop_sessions_active_org_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_account_sessions_account_active_org")
                .table(AccountSessions::Table)
                .to_owned(),
        )
        .await
}

async fn drop_active_org_id_from_sessions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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

#[derive(DeriveIden)]
enum AccountSessions {
    Table,
    AccountId,
    ActiveOrgId,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    OrgId,
    Name,
    CreatedAt,
}
