//! R-0019 migration: add project and project-repository persistence tables.
//!
//! Active-project invariant strategy:
//! - Store active selection as an account-scoped pointer row in
//!   `account_active_projects`.
//! - The `owning_account_id` primary key enforces at most one active-project
//!   pointer per account.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

const REPOSITORY_REF_MAX_LEN: u32 = 140;
const PROVIDER_FAMILY_MAX_LEN: u32 = 48;
const DESIGNATED_HOST_MAX_LEN: u32 = 253;
const PROVIDER_REMOTE_ID_MAX_LEN: u32 = 256;
const PROVIDER_REMOTE_URL_MAX_LEN: u32 = 2048;
const RESERVATION_STATUS_MAX_LEN: u32 = 24;
const PROJECTS_LIST_INDEX: &str = "idx_projects_list_by_account";
const PROJECTS_FRESHNESS_INDEX: &str = "idx_projects_freshness_by_account_created_at";
const PROJECT_REPOSITORIES_PROJECT_LOOKUP_INDEX: &str =
    "idx_project_repositories_owning_account_project";
const PROJECT_COMMAND_RESERVATIONS_BLOCKED_LOOKUP_INDEX: &str =
    "idx_project_command_reservations_blocked_lookup";

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_projects_table(manager).await?;
        create_account_active_projects_table(manager).await?;
        create_project_repositories_table(manager).await?;
        create_project_command_reservations_table(manager).await?;
        create_project_indexes(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(PROJECT_COMMAND_RESERVATIONS_BLOCKED_LOOKUP_INDEX)
                    .table(ProjectCommandReservations::Table)
                    .to_owned(),
            )
            .await?;

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
                    .name(PROJECTS_FRESHNESS_INDEX)
                    .table(Projects::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name(PROJECTS_LIST_INDEX)
                    .table(Projects::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name(PROJECT_REPOSITORIES_PROJECT_LOOKUP_INDEX)
                    .table(ProjectRepositories::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(ProjectRepositories::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(ProjectCommandReservations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(AccountActiveProjects::Table).to_owned())
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

async fn create_account_active_projects_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(AccountActiveProjects::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(AccountActiveProjects::OwningAccountId)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(AccountActiveProjects::ProjectId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(AccountActiveProjects::SelectedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_account_active_projects_owning_account")
                        .from(
                            AccountActiveProjects::Table,
                            AccountActiveProjects::OwningAccountId,
                        )
                        .to(Accounts::Table, Accounts::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .on_update(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_account_active_projects_project")
                        .from(
                            AccountActiveProjects::Table,
                            AccountActiveProjects::ProjectId,
                        )
                        .to(Projects::Table, Projects::Id)
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
                    ColumnDef::new(ProjectRepositories::ProviderRemoteId)
                        .string_len(PROVIDER_REMOTE_ID_MAX_LEN)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectRepositories::ProviderRemoteUrl)
                        .string_len(PROVIDER_REMOTE_URL_MAX_LEN),
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

async fn create_project_command_reservations_table(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProjectCommandReservations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ProjectCommandReservations::OwningAccountId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::ProviderFamily)
                        .string_len(PROVIDER_FAMILY_MAX_LEN)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::RepositoryRef)
                        .string_len(REPOSITORY_REF_MAX_LEN)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::Status)
                        .string_len(RESERVATION_STATUS_MAX_LEN)
                        .not_null(),
                )
                .col(ColumnDef::new(ProjectCommandReservations::ActiveReservationId).uuid())
                .col(
                    ColumnDef::new(ProjectCommandReservations::ReservedAt)
                        .timestamp_with_time_zone(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::LeaseExpiresAt)
                        .timestamp_with_time_zone(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::FailureCount)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::BlockedUntil)
                        .timestamp_with_time_zone(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectCommandReservations::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(ProjectCommandReservations::OwningAccountId)
                        .col(ProjectCommandReservations::ProviderFamily)
                        .col(ProjectCommandReservations::RepositoryRef),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_project_command_reservations_owning_account")
                        .from(
                            ProjectCommandReservations::Table,
                            ProjectCommandReservations::OwningAccountId,
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
    create_projects_list_index(manager).await?;
    create_projects_freshness_index(manager).await?;

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

    manager
        .create_index(
            Index::create()
                .name(PROJECT_REPOSITORIES_PROJECT_LOOKUP_INDEX)
                .table(ProjectRepositories::Table)
                .col(ProjectRepositories::OwningAccountId)
                .col(ProjectRepositories::ProjectId)
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name(PROJECT_COMMAND_RESERVATIONS_BLOCKED_LOOKUP_INDEX)
                .table(ProjectCommandReservations::Table)
                .col(ProjectCommandReservations::BlockedUntil)
                .to_owned(),
        )
        .await
}

async fn create_projects_freshness_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name(PROJECTS_FRESHNESS_INDEX)
                .table(Projects::Table)
                .col(Projects::OwningAccountId)
                .col((Projects::CreatedAt, IndexOrder::Desc))
                .to_owned(),
        )
        .await
}

async fn create_projects_list_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            manager
                .get_connection()
                .execute_unprepared(&format!(
                    "CREATE INDEX IF NOT EXISTS {PROJECTS_LIST_INDEX} ON projects \
                     (owning_account_id, active_selected_at DESC NULLS LAST, created_at DESC, id DESC)"
                ))
                .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite | DatabaseBackend::MySql => {
            manager
                .create_index(
                    Index::create()
                        .name(PROJECTS_LIST_INDEX)
                        .table(Projects::Table)
                        .col(Projects::OwningAccountId)
                        .col((Projects::ActiveSelectedAt, IndexOrder::Desc))
                        .col((Projects::CreatedAt, IndexOrder::Desc))
                        .col((Projects::Id, IndexOrder::Desc))
                        .to_owned(),
                )
                .await
        }
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum AccountActiveProjects {
    Table,
    OwningAccountId,
    ProjectId,
    SelectedAt,
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
    ProviderRemoteId,
    ProviderRemoteUrl,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ProjectCommandReservations {
    Table,
    OwningAccountId,
    ProviderFamily,
    RepositoryRef,
    Status,
    ActiveReservationId,
    ReservedAt,
    LeaseExpiresAt,
    FailureCount,
    BlockedUntil,
    CreatedAt,
    UpdatedAt,
}
