//! R-0020 migration: create `projects`, `specs`, `loops`, `milestones`,
//! `active_projects`, and `project_view_states` tables with account/project
//! scoping indexes.

use sea_orm_migration::prelude::*;

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
        create_projects_table(manager).await?;
        create_specs_table(manager).await?;
        create_loops_table(manager).await?;
        create_milestones_table(manager).await?;
        create_active_projects_table(manager).await?;
        create_project_view_states_table(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_project_view_states(manager).await?;
        drop_active_projects(manager).await?;
        drop_milestones(manager).await?;
        drop_loops(manager).await?;
        drop_specs(manager).await?;
        drop_projects(manager).await?;
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
                .col(ColumnDef::new(Projects::AccountId).uuid().not_null())
                .col(ColumnDef::new(Projects::Name).string().not_null())
                .col(ColumnDef::new(Projects::State).string().not_null())
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
                .name("idx_projects_account_id")
                .table(Projects::Table)
                .col(Projects::AccountId)
                .to_owned(),
        )
        .await
}

async fn create_specs_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Specs::Table)
                .if_not_exists()
                .col(ColumnDef::new(Specs::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Specs::ProjectId).uuid().not_null())
                .col(ColumnDef::new(Specs::Name).string().not_null())
                .col(ColumnDef::new(Specs::NeedsAttention).boolean().not_null())
                .col(ColumnDef::new(Specs::AttentionReason).string())
                .col(
                    ColumnDef::new(Specs::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_specs_project_id")
                .table(Specs::Table)
                .col(Specs::ProjectId)
                .to_owned(),
        )
        .await
}

async fn create_loops_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Loops::Table)
                .if_not_exists()
                .col(ColumnDef::new(Loops::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Loops::ProjectId).uuid().not_null())
                .col(
                    ColumnDef::new(Loops::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_loops_project_id")
                .table(Loops::Table)
                .col(Loops::ProjectId)
                .to_owned(),
        )
        .await
}

async fn create_milestones_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Milestones::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Milestones::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Milestones::ProjectId).uuid().not_null())
                .col(
                    ColumnDef::new(Milestones::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_milestones_project_id")
                .table(Milestones::Table)
                .col(Milestones::ProjectId)
                .to_owned(),
        )
        .await
}

async fn create_active_projects_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
                    ColumnDef::new(ActiveProjects::SwitchedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await
}

async fn create_project_view_states_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProjectViewStates::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ProjectViewStates::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(ProjectViewStates::AccountId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectViewStates::ProjectId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectViewStates::ViewState)
                        .json_binary()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProjectViewStates::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_project_view_states_account_project")
                .table(ProjectViewStates::Table)
                .col(ProjectViewStates::AccountId)
                .col(ProjectViewStates::ProjectId)
                .unique()
                .to_owned(),
        )
        .await
}

async fn drop_project_view_states(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_project_view_states_account_project")
                .table(ProjectViewStates::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(ProjectViewStates::Table).to_owned())
        .await
}

async fn drop_active_projects(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(ActiveProjects::Table).to_owned())
        .await
}

async fn drop_milestones(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_milestones_project_id")
                .table(Milestones::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(Milestones::Table).to_owned())
        .await
}

async fn drop_loops(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_loops_project_id")
                .table(Loops::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(Loops::Table).to_owned())
        .await
}

async fn drop_specs(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_specs_project_id")
                .table(Specs::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(Specs::Table).to_owned())
        .await
}

async fn drop_projects(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_projects_account_id")
                .table(Projects::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(Projects::Table).to_owned())
        .await
}

#[derive(DeriveIden)]
enum Projects {
    Table,
    Id,
    AccountId,
    Name,
    State,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Specs {
    Table,
    Id,
    ProjectId,
    Name,
    NeedsAttention,
    AttentionReason,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Loops {
    Table,
    Id,
    ProjectId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Milestones {
    Table,
    Id,
    ProjectId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ActiveProjects {
    Table,
    AccountId,
    ProjectId,
    SwitchedAt,
}

#[derive(DeriveIden)]
enum ProjectViewStates {
    Table,
    Id,
    AccountId,
    ProjectId,
    ViewState,
    UpdatedAt,
}
