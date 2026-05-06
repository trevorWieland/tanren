//! R-0019 migration: create the `projects` and `active_projects` tables.
//!
//! `projects` stores registered projects with their backing repository
//! identity. A unique index on `repository_identity` enforces the
//! one-project-one-repository constraint (B-0025 / B-0026).
//!
//! `active_projects` stores per-account active-project selection so the
//! last-selected project can be restored on session resume.

use sea_orm_migration::prelude::*;

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
        manager
            .create_table(
                Table::create()
                    .table(Projects::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Projects::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Projects::Name).string().not_null())
                    .col(ColumnDef::new(Projects::RepositoryId).uuid().not_null())
                    .col(ColumnDef::new(Projects::OwnerAccountId).uuid().not_null())
                    .col(ColumnDef::new(Projects::OwnerOrgId).uuid())
                    .col(
                        ColumnDef::new(Projects::RepositoryIdentity)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Projects::RepositoryUrl).string().not_null())
                    .col(
                        ColumnDef::new(Projects::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_projects_repository_identity_unique")
                    .table(Projects::Table)
                    .col(Projects::RepositoryIdentity)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ActiveProjects::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActiveProjects::AccountId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ActiveProjects::ProjectId).uuid().not_null())
                    .col(
                        ColumnDef::new(ActiveProjects::SelectedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ActiveProjects::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_projects_repository_identity_unique")
                    .table(Projects::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Projects::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    Name,
    RepositoryId,
    OwnerAccountId,
    OwnerOrgId,
    RepositoryIdentity,
    RepositoryUrl,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ActiveProjects {
    Table,
    AccountId,
    ProjectId,
    SelectedAt,
}
