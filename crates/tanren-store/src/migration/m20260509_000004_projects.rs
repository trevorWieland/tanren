//! R-0019 migration: add project and project-repository persistence tables.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

const REPOSITORY_REF_MAX_LEN: u32 = 140;
const PROVIDER_FAMILY_MAX_LEN: u32 = 48;
const DESIGNATED_HOST_MAX_LEN: u32 = 253;
const PROJECTS_SINGLE_ACTIVE_PER_ACCOUNT_INDEX: &str = "idx_projects_single_active_per_account";

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_projects_table(manager).await?;
        create_project_repositories_table(manager).await?;
        create_project_indexes(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_project_repositories_owning_account_repo_unique")
                    .table(ProjectRepositories::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_projects_owning_account")
                    .table(Projects::Table)
                    .to_owned(),
            )
            .await?;

        drop_projects_single_active_index(manager).await?;

        manager
            .drop_table(Table::drop().table(ProjectRepositories::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Projects::Table).to_owned())
            .await?;

        Ok(())
    }
}

async fn create_projects_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Projects::Table)
                .if_not_exists()
                .col(ColumnDef::new(Projects::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Projects::OwningAccountId).uuid().not_null())
                .col(
                    ColumnDef::new(Projects::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(Projects::ActiveSelectedAt).timestamp_with_time_zone())
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_projects_owning_account")
                        .from(Projects::Table, Projects::OwningAccountId)
                        .to(Accounts::Table, Accounts::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
}

async fn create_project_repositories_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProjectRepositories::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ProjectRepositories::ProjectId)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(ProjectRepositories::OwningAccountId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectRepositories::RepositoryRef)
                        .string_len(REPOSITORY_REF_MAX_LEN)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectRepositories::ProviderFamily)
                        .string_len(PROVIDER_FAMILY_MAX_LEN)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectRepositories::DesignatedHost)
                        .string_len(DESIGNATED_HOST_MAX_LEN)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectRepositories::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_project_repositories_project")
                        .from(ProjectRepositories::Table, ProjectRepositories::ProjectId)
                        .to(Projects::Table, Projects::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_project_repositories_owning_account")
                        .from(
                            ProjectRepositories::Table,
                            ProjectRepositories::OwningAccountId,
                        )
                        .to(Accounts::Table, Accounts::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
}

async fn create_project_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_projects_owning_account")
                .table(Projects::Table)
                .col(Projects::OwningAccountId)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name("idx_project_repositories_owning_account_repo_unique")
                .table(ProjectRepositories::Table)
                .col(ProjectRepositories::OwningAccountId)
                .col(ProjectRepositories::ProviderFamily)
                .col(ProjectRepositories::RepositoryRef)
                .unique()
                .to_owned(),
        )
        .await?;

    create_projects_single_active_index(manager).await
}

async fn create_projects_single_active_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    match backend {
        DatabaseBackend::Sqlite | DatabaseBackend::Postgres => {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "CREATE UNIQUE INDEX IF NOT EXISTS {PROJECTS_SINGLE_ACTIVE_PER_ACCOUNT_INDEX} \
                     ON projects (owning_account_id) WHERE active_selected_at IS NOT NULL"
                ))
                .await?;
            Ok(())
        }
        DatabaseBackend::MySql => Ok(()),
    }
}

async fn drop_projects_single_active_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    if matches!(backend, DatabaseBackend::Sqlite | DatabaseBackend::Postgres) {
        manager
            .get_connection()
            .execute_unprepared(&format!(
                "DROP INDEX IF EXISTS {PROJECTS_SINGLE_ACTIVE_PER_ACCOUNT_INDEX}"
            ))
            .await?;
    }
    Ok(())
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    OwningAccountId,
    CreatedAt,
    ActiveSelectedAt,
}

#[derive(DeriveIden)]
enum ProjectRepositories {
    Table,
    ProjectId,
    OwningAccountId,
    RepositoryRef,
    ProviderFamily,
    DesignatedHost,
    CreatedAt,
}
