//! Initial schema — the three core tables and their indexes.
//!
//! Creates `events`, `dispatch_projection`, and `step_projection` plus
//! every index listed in the Lane 0.3 spec. All JSON columns use
//! [`ColumnDef::json_binary`], which emits `TEXT` on `SQLite` and
//! `JSONB` on `Postgres` — backend-agnostic JSON support from a
//! single definition.

use sea_orm_migration::prelude::*;

/// Migration implementation — `up` creates every table and index,
/// `down` drops them in the reverse order.
#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0001_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(create_events_table(manager)).await?;
        Box::pin(create_dispatch_projection_table(manager)).await?;
        Box::pin(create_step_projection_table(manager)).await?;
        Box::pin(create_events_indexes(manager)).await?;
        Box::pin(create_dispatch_projection_indexes(manager)).await?;
        Box::pin(create_step_projection_indexes(manager)).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(drop_step_projection_indexes(manager)).await?;
        Box::pin(drop_dispatch_projection_indexes(manager)).await?;
        Box::pin(drop_events_indexes(manager)).await?;
        manager
            .drop_table(Table::drop().table(StepProjection::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DispatchProjection::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Events::Table).to_owned())
            .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Table creation
// ---------------------------------------------------------------------------

async fn create_events_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Events::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Events::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Events::EventId)
                        .uuid()
                        .not_null()
                        .unique_key(),
                )
                .col(
                    ColumnDef::new(Events::Timestamp)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(ColumnDef::new(Events::EntityKind).string().not_null())
                .col(ColumnDef::new(Events::EntityId).string().not_null())
                .col(ColumnDef::new(Events::EventType).string().not_null())
                .col(ColumnDef::new(Events::SchemaVersion).integer().not_null())
                .col(ColumnDef::new(Events::Payload).json_binary().not_null())
                .to_owned(),
        )
        .await
}

async fn create_dispatch_projection_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(DispatchProjection::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(DispatchProjection::DispatchId)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(DispatchProjection::Mode).string().not_null())
                .col(
                    ColumnDef::new(DispatchProjection::Status)
                        .string()
                        .not_null(),
                )
                .col(ColumnDef::new(DispatchProjection::Outcome).string().null())
                .col(ColumnDef::new(DispatchProjection::Lane).string().not_null())
                .col(
                    ColumnDef::new(DispatchProjection::Dispatch)
                        .json_binary()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DispatchProjection::Actor)
                        .json_binary()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DispatchProjection::GraphRevision)
                        .integer()
                        .not_null(),
                )
                .col(ColumnDef::new(DispatchProjection::UserId).uuid().not_null())
                .col(
                    ColumnDef::new(DispatchProjection::Project)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DispatchProjection::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DispatchProjection::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await
}

async fn create_step_projection_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(StepProjection::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(StepProjection::StepId)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(StepProjection::DispatchId).uuid().not_null())
                .col(ColumnDef::new(StepProjection::StepType).string().not_null())
                .col(
                    ColumnDef::new(StepProjection::StepSequence)
                        .integer()
                        .not_null(),
                )
                .col(ColumnDef::new(StepProjection::Lane).string().null())
                .col(ColumnDef::new(StepProjection::Status).string().not_null())
                .col(
                    ColumnDef::new(StepProjection::ReadyState)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(StepProjection::DependsOn)
                        .json_binary()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(StepProjection::GraphRevision)
                        .integer()
                        .not_null(),
                )
                .col(ColumnDef::new(StepProjection::WorkerId).string().null())
                .col(ColumnDef::new(StepProjection::Payload).json_binary().null())
                .col(ColumnDef::new(StepProjection::Result).json_binary().null())
                .col(ColumnDef::new(StepProjection::Error).string().null())
                .col(
                    ColumnDef::new(StepProjection::RetryCount)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(StepProjection::LastHeartbeatAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .col(
                    ColumnDef::new(StepProjection::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(StepProjection::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await
}

// ---------------------------------------------------------------------------
// Index creation
// ---------------------------------------------------------------------------

async fn create_events_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_events_entity_id")
                .table(Events::Table)
                .col(Events::EntityId)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_events_entity_kind")
                .table(Events::Table)
                .col(Events::EntityKind)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_events_event_type")
                .table(Events::Table)
                .col(Events::EventType)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_events_timestamp")
                .table(Events::Table)
                .col(Events::Timestamp)
                .to_owned(),
        )
        .await?;
    Ok(())
}

async fn create_dispatch_projection_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for (name, col) in [
        ("ix_dispatch_status", DispatchProjection::Status),
        ("ix_dispatch_lane", DispatchProjection::Lane),
        ("ix_dispatch_created", DispatchProjection::CreatedAt),
        ("ix_dispatch_user", DispatchProjection::UserId),
        ("ix_dispatch_project", DispatchProjection::Project),
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
    // S-01: composite index for `query_dispatches(status=?, ...) ORDER BY
    // created_at DESC` — without it, the planner spills to a temp B-tree
    // for the sort.
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_dispatch_status_created")
                .table(DispatchProjection::Table)
                .col(DispatchProjection::Status)
                .col(DispatchProjection::CreatedAt)
                .to_owned(),
        )
        .await?;
    Ok(())
}

async fn create_step_projection_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for (name, col) in [
        ("ix_step_dispatch", StepProjection::DispatchId),
        ("ix_step_status", StepProjection::Status),
    ] {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name(name)
                    .table(StepProjection::Table)
                    .col(col)
                    .to_owned(),
            )
            .await?;
    }
    // Composite (lane, status) for lane-scoped counts and dequeue
    // candidate selection.
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_step_lane_status")
                .table(StepProjection::Table)
                .col(StepProjection::Lane)
                .col(StepProjection::Status)
                .to_owned(),
        )
        .await?;
    // Composite (status, created_at) to support the dequeue ORDER BY.
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
        .await?;
    // S-01: composite index for `get_steps_for_dispatch` — without it,
    // the planner does a (dispatch_id) index scan followed by a temp
    // B-tree sort on step_sequence.
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_step_dispatch_sequence")
                .table(StepProjection::Table)
                .col(StepProjection::DispatchId)
                .col(StepProjection::StepSequence)
                .to_owned(),
        )
        .await?;
    // Liveness scan for `recover_stale_steps`: find running rows
    // whose heartbeat is older than the threshold.
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_step_status_heartbeat")
                .table(StepProjection::Table)
                .col(StepProjection::Status)
                .col(StepProjection::LastHeartbeatAt)
                .to_owned(),
        )
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Index drops (down migration)
// ---------------------------------------------------------------------------

async fn drop_events_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in [
        "ix_events_timestamp",
        "ix_events_event_type",
        "ix_events_entity_kind",
        "ix_events_entity_id",
    ] {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(name)
                    .table(Events::Table)
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

async fn drop_dispatch_projection_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in [
        "ix_dispatch_status_created",
        "ix_dispatch_project",
        "ix_dispatch_user",
        "ix_dispatch_created",
        "ix_dispatch_lane",
        "ix_dispatch_status",
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

async fn drop_step_projection_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in [
        "ix_step_status_heartbeat",
        "ix_step_dispatch_sequence",
        "ix_step_status_created",
        "ix_step_lane_status",
        "ix_step_status",
        "ix_step_dispatch",
    ] {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name(name)
                    .table(StepProjection::Table)
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Column identifiers
// ---------------------------------------------------------------------------

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
    EventId,
    Timestamp,
    EntityKind,
    EntityId,
    EventType,
    SchemaVersion,
    Payload,
}

#[derive(DeriveIden)]
enum DispatchProjection {
    Table,
    DispatchId,
    Mode,
    Status,
    Outcome,
    Lane,
    Dispatch,
    Actor,
    GraphRevision,
    UserId,
    Project,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum StepProjection {
    Table,
    StepId,
    DispatchId,
    StepType,
    StepSequence,
    Lane,
    Status,
    ReadyState,
    DependsOn,
    GraphRevision,
    WorkerId,
    Payload,
    Result,
    Error,
    RetryCount,
    LastHeartbeatAt,
    CreatedAt,
    UpdatedAt,
}
