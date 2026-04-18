use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0012_methodology_audit_pipeline"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_events_spec_id_column_if_missing(manager).await?;
        create_events_spec_index(manager).await?;
        create_phase_event_outbox_table(manager).await?;
        create_methodology_idempotency_table(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // `events.spec_id` is intentionally left in place: dropping
        // columns is not backend-portable in SQLite without table
        // rebuilds. The extra column is benign for older callers.
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_events_spec_id_ts")
                    .table(Events::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_outbox_spec_status")
                    .table(MethodologyPhaseEventOutbox::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_outbox_status_created")
                    .table(MethodologyPhaseEventOutbox::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_idempotency_updated")
                    .table(MethodologyIdempotency::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MethodologyPhaseEventOutbox::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MethodologyIdempotency::Table)
                    .to_owned(),
            )
            .await
    }
}

async fn add_events_spec_id_column_if_missing(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if manager.has_column("events", "spec_id").await? {
        return Ok(());
    }
    manager
        .alter_table(
            Table::alter()
                .table(Events::Table)
                .add_column(ColumnDef::new(Events::SpecId).uuid().null().to_owned())
                .to_owned(),
        )
        .await
}

async fn create_events_spec_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_events_spec_id_ts")
                .table(Events::Table)
                .col(Events::SpecId)
                .col(Events::EventType)
                .col(Events::Timestamp)
                .col(Events::Id)
                .to_owned(),
        )
        .await
}

async fn create_phase_event_outbox_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .if_not_exists()
                .table(MethodologyPhaseEventOutbox::Table)
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::EventId)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::SpecId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::SpecFolder)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::LineJson)
                        .text()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::Status)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::AttemptCount)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::LastError)
                        .text()
                        .null(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyPhaseEventOutbox::ProjectedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_methodology_outbox_status_created")
                .table(MethodologyPhaseEventOutbox::Table)
                .col(MethodologyPhaseEventOutbox::Status)
                .col(MethodologyPhaseEventOutbox::CreatedAt)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_methodology_outbox_spec_status")
                .table(MethodologyPhaseEventOutbox::Table)
                .col(MethodologyPhaseEventOutbox::SpecId)
                .col(MethodologyPhaseEventOutbox::Status)
                .col(MethodologyPhaseEventOutbox::CreatedAt)
                .to_owned(),
        )
        .await
}

async fn create_methodology_idempotency_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .if_not_exists()
                .table(MethodologyIdempotency::Table)
                .col(
                    ColumnDef::new(MethodologyIdempotency::Tool)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::ScopeKey)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::IdempotencyKey)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::RequestHash)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::ResponseJson)
                        .text()
                        .null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::FirstEventId)
                        .uuid()
                        .null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(MethodologyIdempotency::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(MethodologyIdempotency::Tool)
                        .col(MethodologyIdempotency::ScopeKey)
                        .col(MethodologyIdempotency::IdempotencyKey),
                )
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name("ix_methodology_idempotency_updated")
                .table(MethodologyIdempotency::Table)
                .col(MethodologyIdempotency::UpdatedAt)
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
    Timestamp,
    EventType,
    SpecId,
}

#[derive(DeriveIden)]
enum MethodologyPhaseEventOutbox {
    Table,
    EventId,
    SpecId,
    SpecFolder,
    LineJson,
    Status,
    AttemptCount,
    LastError,
    CreatedAt,
    ProjectedAt,
}

#[derive(DeriveIden)]
enum MethodologyIdempotency {
    Table,
    Tool,
    ScopeKey,
    IdempotencyKey,
    RequestHash,
    ResponseJson,
    FirstEventId,
    CreatedAt,
    UpdatedAt,
}
