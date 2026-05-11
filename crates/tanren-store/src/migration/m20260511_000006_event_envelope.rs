//! State architecture event-envelope expansion. Adds envelope columns to the
//! canonical `events` table so every durable mutation carries the metadata
//! the state subsystem contract mandates: global position, event type,
//! schema version, actor, scope, resource, correlation/causation/idempotency,
//! and visibility.
//!
//! Envelope columns other than `position` are nullable so existing rows
//! remain valid; new writes populate every field. The `position` column
//! is NOT NULL — the database assigns a globally monotonic value on every
//! insert via the `events_position_seq` sequence. Existing rows are
//! backfilled from the same sequence so the column is the canonical
//! replay-ordering authority per docs/architecture/subsystems/state.md
//! § Event Log.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

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
        let backend = manager.get_database_backend();
        create_position_sequence(manager, &backend).await?;
        add_position_column(manager).await?;
        backfill_and_constrain_position(manager, &backend).await?;
        add_envelope_columns(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let cols = [
            Events::Visibility,
            Events::IdempotencyKey,
            Events::CausationId,
            Events::CorrelationId,
            Events::ResourceId,
            Events::Scope,
            Events::Actor,
            Events::SchemaVersion,
            Events::EventType,
            Events::Position,
        ];
        for col in cols {
            manager
                .alter_table(
                    Table::alter()
                        .table(Events::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        let backend = manager.get_database_backend();
        if matches!(backend, DatabaseBackend::Postgres) {
            manager
                .get_connection()
                .execute_unprepared("DROP SEQUENCE IF EXISTS events_position_seq")
                .await?;
        }
        Ok(())
    }
}

async fn create_position_sequence(
    manager: &SchemaManager<'_>,
    backend: &DatabaseBackend,
) -> Result<(), DbErr> {
    if matches!(backend, DatabaseBackend::Postgres) {
        manager
            .get_connection()
            .execute_unprepared("CREATE SEQUENCE IF NOT EXISTS events_position_seq")
            .await?;
    }
    Ok(())
}

async fn add_position_column(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Events::Table)
                .add_column(
                    ColumnDef::new(Events::Position)
                        .big_integer()
                        .default(Expr::cust("nextval('events_position_seq')")),
                )
                .to_owned(),
        )
        .await
}

/// Backfill existing rows with positions from the same sequence, then
/// make the column NOT NULL. Every row must carry a database-assigned
/// global position — the canonical replay-ordering authority.
async fn backfill_and_constrain_position(
    manager: &SchemaManager<'_>,
    backend: &DatabaseBackend,
) -> Result<(), DbErr> {
    if !matches!(backend, DatabaseBackend::Postgres) {
        return Ok(());
    }
    manager
        .get_connection()
        .execute_unprepared(
            "UPDATE events SET position = nextval('events_position_seq') \
             WHERE position IS NULL ORDER BY occurred_at, id",
        )
        .await?;
    manager
        .alter_table(
            Table::alter()
                .table(Events::Table)
                .modify_column(ColumnDef::new(Events::Position).big_integer().not_null())
                .to_owned(),
        )
        .await
}

async fn add_envelope_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let alterations: Vec<(Events, ColumnType)> = vec![
        (Events::EventType, ColumnType::String(StringLen::None)),
        (Events::SchemaVersion, ColumnType::String(StringLen::None)),
        (Events::Actor, ColumnType::String(StringLen::None)),
        (Events::Scope, ColumnType::String(StringLen::None)),
        (Events::ResourceId, ColumnType::Uuid),
        (Events::CorrelationId, ColumnType::Uuid),
        (Events::CausationId, ColumnType::Uuid),
        (Events::IdempotencyKey, ColumnType::String(StringLen::None)),
        (Events::Visibility, ColumnType::String(StringLen::None)),
    ];
    for (col, _ty) in alterations {
        manager
            .alter_table(
                Table::alter()
                    .table(Events::Table)
                    .add_column(ColumnDef::new(col).string())
                    .to_owned(),
            )
            .await?;
    }
    Ok(())
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Position,
    EventType,
    SchemaVersion,
    Actor,
    Scope,
    ResourceId,
    CorrelationId,
    CausationId,
    IdempotencyKey,
    Visibility,
}
