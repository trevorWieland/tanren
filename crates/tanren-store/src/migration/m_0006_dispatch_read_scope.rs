//! Dispatch projection read-scope columns and indexes.
//!
//! Adds denormalized actor scope fields used for policy-aligned
//! list/read authorization filters:
//! - `org_id`
//! - `scope_project_id`
//! - `scope_team_id`
//! - `scope_api_key_id`

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0006_dispatch_read_scope"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(add_scope_columns(manager)).await?;
        Box::pin(backfill_scope_columns(manager)).await?;
        Box::pin(create_scope_indexes(manager)).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // We intentionally drop only indexes here. Full `Migrator::down`
        // will eventually drop `dispatch_projection` in m_0001.
        Box::pin(drop_scope_indexes(manager)).await
    }
}

async fn add_scope_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    add_column_if_missing(
        manager,
        DispatchProjection::OrgId,
        ColumnDef::new(DispatchProjection::OrgId)
            .uuid()
            .null()
            .to_owned(),
    )
    .await?;
    add_column_if_missing(
        manager,
        DispatchProjection::ScopeProjectId,
        ColumnDef::new(DispatchProjection::ScopeProjectId)
            .uuid()
            .null()
            .to_owned(),
    )
    .await?;
    add_column_if_missing(
        manager,
        DispatchProjection::ScopeTeamId,
        ColumnDef::new(DispatchProjection::ScopeTeamId)
            .uuid()
            .null()
            .to_owned(),
    )
    .await?;
    add_column_if_missing(
        manager,
        DispatchProjection::ScopeApiKeyId,
        ColumnDef::new(DispatchProjection::ScopeApiKeyId)
            .uuid()
            .null()
            .to_owned(),
    )
    .await?;
    Ok(())
}

async fn add_column_if_missing(
    manager: &SchemaManager<'_>,
    column: DispatchProjection,
    column_def: ColumnDef,
) -> Result<(), DbErr> {
    if manager
        .has_column("dispatch_projection", scope_column_name(column))
        .await?
    {
        return Ok(());
    }
    manager
        .alter_table(
            Table::alter()
                .table(DispatchProjection::Table)
                .add_column(column_def)
                .to_owned(),
        )
        .await
}

fn scope_column_name(column: DispatchProjection) -> &'static str {
    match column {
        DispatchProjection::OrgId => "org_id",
        DispatchProjection::ScopeProjectId => "scope_project_id",
        DispatchProjection::ScopeTeamId => "scope_team_id",
        DispatchProjection::ScopeApiKeyId => "scope_api_key_id",
        DispatchProjection::Table
        | DispatchProjection::DispatchId
        | DispatchProjection::CreatedAt => {
            unreachable!("non-scope column requested for add_column_if_missing")
        }
    }
}

async fn backfill_scope_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if is_postgres(manager) {
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE dispatch_projection
                 SET org_id = NULLIF(actor ->> 'org_id', '')::uuid,
                     scope_project_id = NULLIF(actor ->> 'project_id', '')::uuid,
                     scope_team_id = NULLIF(actor ->> 'team_id', '')::uuid,
                     scope_api_key_id = NULLIF(actor ->> 'api_key_id', '')::uuid
                 WHERE org_id IS NULL",
            )
            .await?;
        return Ok(());
    }

    manager
        .get_connection()
        .execute_unprepared(
            "UPDATE dispatch_projection
             SET org_id = json_extract(actor, '$.org_id'),
                 scope_project_id = json_extract(actor, '$.project_id'),
                 scope_team_id = json_extract(actor, '$.team_id'),
                 scope_api_key_id = json_extract(actor, '$.api_key_id')
             WHERE org_id IS NULL",
        )
        .await?;
    Ok(())
}

async fn create_scope_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_dispatch_org_created_dispatch")
                .table(DispatchProjection::Table)
                .col(DispatchProjection::OrgId)
                .col(DispatchProjection::CreatedAt)
                .col(DispatchProjection::DispatchId)
                .to_owned(),
        )
        .await?;

    for (name, col) in [
        (
            "ix_dispatch_scope_project",
            DispatchProjection::ScopeProjectId,
        ),
        ("ix_dispatch_scope_team", DispatchProjection::ScopeTeamId),
        (
            "ix_dispatch_scope_api_key",
            DispatchProjection::ScopeApiKeyId,
        ),
    ] {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(name)
                    .table(DispatchProjection::Table)
                    .col(col)
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

async fn drop_scope_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in [
        "ix_dispatch_scope_api_key",
        "ix_dispatch_scope_team",
        "ix_dispatch_scope_project",
        "ix_dispatch_org_created_dispatch",
    ] {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(name)
                    .table(DispatchProjection::Table)
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
    matches!(manager.get_database_backend(), sea_orm::DbBackend::Postgres)
}

#[derive(Clone, Copy, DeriveIden)]
enum DispatchProjection {
    Table,
    DispatchId,
    CreatedAt,
    OrgId,
    ScopeProjectId,
    ScopeTeamId,
    ScopeApiKeyId,
}
