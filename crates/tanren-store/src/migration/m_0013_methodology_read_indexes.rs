use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0013_methodology_read_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_events_spec_kind_type_ts")
                    .table(Events::Table)
                    .col(Events::SpecId)
                    .col(Events::EntityKind)
                    .col(Events::EventType)
                    .col(Events::Timestamp)
                    .col(Events::Id)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_events_kind_id_type_ts")
                    .table(Events::Table)
                    .col(Events::EntityKind)
                    .col(Events::EntityId)
                    .col(Events::EventType)
                    .col(Events::Timestamp)
                    .col(Events::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_events_kind_id_type_ts")
                    .table(Events::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_events_spec_kind_type_ts")
                    .table(Events::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
    SpecId,
    EntityKind,
    EntityId,
    EventType,
    Timestamp,
}
