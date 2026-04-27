//! Composite scope index for high-cardinality scoped dispatch reads.

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0007_dispatch_scope_tuple_index"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_scope_tuple_created_dispatch")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::OrgId)
                    .col(DispatchProjection::ScopeProjectId)
                    .col(DispatchProjection::ScopeTeamId)
                    .col(DispatchProjection::ScopeApiKeyId)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_dispatch_scope_tuple_created_dispatch")
                    .table(DispatchProjection::Table)
                    .to_owned(),
            )
            .await
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
