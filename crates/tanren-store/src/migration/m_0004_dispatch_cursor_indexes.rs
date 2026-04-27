//! Cursor pagination indexes for dispatch queries.
//!
//! Supports keyset pagination with ordering:
//! `ORDER BY created_at DESC, dispatch_id DESC`.

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0004_dispatch_cursor_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_created_dispatch_id")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_status_created_dispatch_id")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::Status)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_lane_created_dispatch_id")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::Lane)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_dispatch_project_created_dispatch_id")
                    .table(DispatchProjection::Table)
                    .col(DispatchProjection::Project)
                    .col(DispatchProjection::CreatedAt)
                    .col(DispatchProjection::DispatchId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in [
            "ix_dispatch_project_created_dispatch_id",
            "ix_dispatch_lane_created_dispatch_id",
            "ix_dispatch_status_created_dispatch_id",
            "ix_dispatch_created_dispatch_id",
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
    Status,
    Lane,
    Project,
    CreatedAt,
}
