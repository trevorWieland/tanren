//! R-0021 migration: create `projects`, `project_specs`,
//! `project_dependencies`, and `project_loop_fixtures` tables.
//!
//! Disconnected projects and their specs are retained (soft-delete via
//! `disconnected_at`) so that reconnection via B-0025 can restore access
//! to prior specs. A partial unique index on `(org_id, provider_connection_id, resource_id)`
//! filtered to `disconnected_at IS NULL` ensures at most one active
//! connection per repository per organisation while allowing the same
//! repository to reconnect after disconnect.

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

impl Migration {
    async fn create_projects_table(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Projects::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Projects::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Projects::OrgId).uuid().not_null())
                    .col(ColumnDef::new(Projects::Name).string().not_null())
                    .col(
                        ColumnDef::new(Projects::ProviderConnectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Projects::ResourceId).string().not_null())
                    .col(ColumnDef::new(Projects::DisplayRef).string().not_null())
                    .col(
                        ColumnDef::new(Projects::ConnectedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Projects::DisconnectedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        let backend = db.get_database_backend();
        db.execute(Statement::from_string(
                backend,
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_active_repo_per_org ON projects (org_id, provider_connection_id, resource_id) WHERE disconnected_at IS NULL".to_owned(),
            ))
            .await?;
        Ok(())
    }

    async fn create_project_specs_table(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectSpecs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectSpecs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ProjectSpecs::ProjectId).uuid().not_null())
                    .col(ColumnDef::new(ProjectSpecs::Title).string().not_null())
                    .col(ColumnDef::new(ProjectSpecs::Body).text().not_null())
                    .col(
                        ColumnDef::new(ProjectSpecs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_specs_project_id")
                    .table(ProjectSpecs::Table)
                    .col(ProjectSpecs::ProjectId)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn create_project_dependencies_table(
        &self,
        manager: &SchemaManager<'_>,
    ) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectDependencies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectDependencies::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProjectDependencies::SourceProjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProjectDependencies::SourceSpecId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProjectDependencies::TargetProjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProjectDependencies::DetectedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_dependencies_source_project")
                    .table(ProjectDependencies::Table)
                    .col(ProjectDependencies::SourceProjectId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_dependencies_target_project")
                    .table(ProjectDependencies::Table)
                    .col(ProjectDependencies::TargetProjectId)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn create_project_loop_fixtures_table(
        &self,
        manager: &SchemaManager<'_>,
    ) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectLoopFixtures::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProjectLoopFixtures::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProjectLoopFixtures::ProjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProjectLoopFixtures::IsActive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ProjectLoopFixtures::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_project_loop_fixtures_project_id")
                    .table(ProjectLoopFixtures::Table)
                    .col(ProjectLoopFixtures::ProjectId)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        self.create_projects_table(manager).await?;
        self.create_project_specs_table(manager).await?;
        self.create_project_dependencies_table(manager).await?;
        self.create_project_loop_fixtures_table(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProjectLoopFixtures::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ProjectDependencies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ProjectSpecs::Table).to_owned())
            .await?;
        {
            let db = manager.get_connection();
            let backend = db.get_database_backend();
            db.execute(Statement::from_string(
                backend,
                "DROP INDEX IF EXISTS idx_projects_active_repo_per_org".to_owned(),
            ))
            .await?;
        }
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
    OrgId,
    Name,
    ProviderConnectionId,
    ResourceId,
    DisplayRef,
    ConnectedAt,
    DisconnectedAt,
}

#[derive(DeriveIden)]
enum ProjectSpecs {
    Table,
    Id,
    ProjectId,
    Title,
    Body,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ProjectDependencies {
    Table,
    Id,
    SourceProjectId,
    SourceSpecId,
    TargetProjectId,
    DetectedAt,
}

#[derive(DeriveIden)]
enum ProjectLoopFixtures {
    Table,
    Id,
    ProjectId,
    IsActive,
    CreatedAt,
}
