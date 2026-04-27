//! Integrity hardening — FK constraints, unique indexes, and
//! composite indexes that were missing from the initial schema.
//!
//! Addresses audit findings:
//! - **P1**: orphan step rows (FK on `step_projection.dispatch_id`)
//! - **P2**: advisory step ordering (unique on `(dispatch_id, step_sequence)`)
//! - **P2**: temp-sort spills on `query_events` and `query_dispatches(lane)`

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0002_integrity"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(create_step_dispatch_fk(manager)).await?;
        Box::pin(create_unique_step_sequence(manager)).await?;
        Box::pin(create_events_composite_index(manager)).await?;
        Box::pin(create_dispatch_lane_created_index(manager)).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(drop_dispatch_lane_created_index(manager)).await?;
        Box::pin(drop_events_composite_index(manager)).await?;
        Box::pin(drop_unique_step_sequence(manager)).await?;
        Box::pin(drop_step_dispatch_fk(manager)).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Foreign key: step_projection.dispatch_id -> dispatch_projection.dispatch_id
// ---------------------------------------------------------------------------

/// Add FK `step_projection.dispatch_id → dispatch_projection.dispatch_id`.
///
/// `SQLite` does not support `ALTER TABLE ... ADD FOREIGN KEY`, so this
/// constraint is only added on Postgres. On `SQLite`, the application-
/// level existence check in `enqueue_step` (`job_queue.rs`) is the
/// primary defense, backed by `PRAGMA foreign_keys = ON` for any
/// future table recreations.
async fn create_step_dispatch_fk(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if !is_postgres(manager) {
        return Ok(());
    }
    manager
        .create_foreign_key(
            ForeignKey::create()
                .name("fk_step_dispatch")
                .from(StepProjection::Table, StepProjection::DispatchId)
                .to(DispatchProjection::Table, DispatchProjection::DispatchId)
                .on_delete(ForeignKeyAction::Restrict)
                .on_update(ForeignKeyAction::Cascade)
                .to_owned(),
        )
        .await
}

async fn drop_step_dispatch_fk(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if !is_postgres(manager) {
        return Ok(());
    }
    manager
        .drop_foreign_key(
            ForeignKey::drop()
                .name("fk_step_dispatch")
                .table(StepProjection::Table)
                .to_owned(),
        )
        .await
}

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
    matches!(manager.get_database_backend(), sea_orm::DbBackend::Postgres,)
}

// ---------------------------------------------------------------------------
// Unique index: (dispatch_id, step_sequence)
// ---------------------------------------------------------------------------

async fn create_unique_step_sequence(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("uq_step_dispatch_sequence")
                .table(StepProjection::Table)
                .col(StepProjection::DispatchId)
                .col(StepProjection::StepSequence)
                .unique()
                .to_owned(),
        )
        .await
}

async fn drop_unique_step_sequence(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("uq_step_dispatch_sequence")
                .table(StepProjection::Table)
                .to_owned(),
        )
        .await
}

// ---------------------------------------------------------------------------
// Composite index: events (entity_kind, entity_id, timestamp, id)
// Eliminates temp sort for query_events(entity_kind + entity_id
// ORDER BY timestamp, id).
// ---------------------------------------------------------------------------

async fn create_events_composite_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_events_entity_kind_id_ts")
                .table(Events::Table)
                .col(Events::EntityKind)
                .col(Events::EntityId)
                .col(Events::Timestamp)
                .col(Events::Id)
                .to_owned(),
        )
        .await
}

async fn drop_events_composite_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("ix_events_entity_kind_id_ts")
                .table(Events::Table)
                .to_owned(),
        )
        .await
}

// ---------------------------------------------------------------------------
// Composite index: dispatch_projection (lane, created_at)
// Eliminates temp sort for query_dispatches(lane) ORDER BY created_at.
// ---------------------------------------------------------------------------

async fn create_dispatch_lane_created_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_dispatch_lane_created")
                .table(DispatchProjection::Table)
                .col(DispatchProjection::Lane)
                .col(DispatchProjection::CreatedAt)
                .to_owned(),
        )
        .await
}

async fn drop_dispatch_lane_created_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .if_exists()
                .name("ix_dispatch_lane_created")
                .table(DispatchProjection::Table)
                .to_owned(),
        )
        .await
}

// ---------------------------------------------------------------------------
// Column identifiers (each migration defines its own per SeaORM convention)
// ---------------------------------------------------------------------------

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
    EntityKind,
    EntityId,
    Timestamp,
}

#[derive(DeriveIden)]
enum DispatchProjection {
    Table,
    DispatchId,
    Lane,
    CreatedAt,
}

#[derive(DeriveIden)]
enum StepProjection {
    Table,
    DispatchId,
    StepSequence,
}
