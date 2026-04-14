//! Covering indexes for the dequeue hot path.
//!
//! The original `(status, created_at)` index does not cover
//! `ready_state` or `step_sequence`, causing `SQLite` to spill to a
//! temp B-tree for the ORDER BY. This migration adds indexes that
//! match the real dequeue predicate and ordering, and drops the
//! subsumed `ix_step_status_created`.
//!
//! Addresses audit finding I-02.

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0003_dequeue_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(create_dequeue_global_index(manager)).await?;
        Box::pin(create_dequeue_lane_index(manager)).await?;
        Box::pin(drop_subsumed_index(manager)).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(restore_subsumed_index(manager)).await?;
        Box::pin(drop_dequeue_lane_index(manager)).await?;
        Box::pin(drop_dequeue_global_index(manager)).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Global dequeue index
// ---------------------------------------------------------------------------

async fn create_dequeue_global_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if is_postgres(manager) {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS ix_step_dequeue_global \
                 ON step_projection (created_at, step_sequence) \
                 WHERE status = 'pending' AND ready_state = 'ready'",
            )
            .await?;
        return Ok(());
    }
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_step_dequeue_global")
                .table(StepProjection::Table)
                .col(StepProjection::Status)
                .col(StepProjection::ReadyState)
                .col(StepProjection::CreatedAt)
                .col(StepProjection::StepSequence)
                .to_owned(),
        )
        .await
}

async fn drop_dequeue_global_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("ix_step_dequeue_global")
                .table(StepProjection::Table)
                .to_owned(),
        )
        .await
}

// ---------------------------------------------------------------------------
// Lane-scoped dequeue index
// ---------------------------------------------------------------------------

async fn create_dequeue_lane_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if is_postgres(manager) {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS ix_step_dequeue_lane \
                 ON step_projection (lane, created_at, step_sequence) \
                 WHERE status = 'pending' AND ready_state = 'ready'",
            )
            .await?;
        return Ok(());
    }
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_step_dequeue_lane")
                .table(StepProjection::Table)
                .col(StepProjection::Lane)
                .col(StepProjection::Status)
                .col(StepProjection::ReadyState)
                .col(StepProjection::CreatedAt)
                .col(StepProjection::StepSequence)
                .to_owned(),
        )
        .await
}

async fn drop_dequeue_lane_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("ix_step_dequeue_lane")
                .table(StepProjection::Table)
                .to_owned(),
        )
        .await
}

// ---------------------------------------------------------------------------
// Drop subsumed ix_step_status_created
// ---------------------------------------------------------------------------

async fn drop_subsumed_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("ix_step_status_created")
                .table(StepProjection::Table)
                .to_owned(),
        )
        .await
}

async fn restore_subsumed_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_step_status_created")
                .table(StepProjection::Table)
                .col(StepProjection::Status)
                .col(StepProjection::CreatedAt)
                .to_owned(),
        )
        .await
}

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
    matches!(manager.get_database_backend(), sea_orm::DbBackend::Postgres)
}

// ---------------------------------------------------------------------------
// Column identifiers
// ---------------------------------------------------------------------------

#[derive(DeriveIden)]
enum StepProjection {
    Table,
    Status,
    ReadyState,
    CreatedAt,
    StepSequence,
    Lane,
}
