//! Composite scope indexes for common scoped-read tuples.
//!
//! `m_0007` added the full `(org, project, team, api_key, created_at, dispatch_id)`
//! tuple index. This migration adds narrower prefixes used by common actor scopes.

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0008_dispatch_scope_common_tuple_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_scope_org_project_created_dispatch")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::OrgId)
                    .col(DispatchProjection::ScopeProjectId)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_scope_org_project_team_created_dispatch")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::OrgId)
                    .col(DispatchProjection::ScopeProjectId)
                    .col(DispatchProjection::ScopeTeamId)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_scope_org_project_api_key_created_dispatch")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::OrgId)
                    .col(DispatchProjection::ScopeProjectId)
                    .col(DispatchProjection::ScopeApiKeyId)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in [
            "ix_dispatch_scope_org_project_api_key_created_dispatch",
            "ix_dispatch_scope_org_project_team_created_dispatch",
            "ix_dispatch_scope_org_project_created_dispatch",
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
}

#[derive(DeriveIden)]
enum DispatchProjection {
    Table,
    DispatchId,
    CreatedAt,
    OrgId,
    ScopeProjectId,
    ScopeTeamId,
    ScopeApiKeyId,
}
