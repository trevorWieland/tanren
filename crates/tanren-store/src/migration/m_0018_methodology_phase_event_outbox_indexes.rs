use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0018_methodology_phase_event_outbox_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_methodology_outbox_spec_status_created_event_id")
                    .table(MethodologyPhaseEventOutbox::Table)
                    .col(MethodologyPhaseEventOutbox::SpecId)
                    .col(MethodologyPhaseEventOutbox::Status)
                    .col(MethodologyPhaseEventOutbox::CreatedAt)
                    .col(MethodologyPhaseEventOutbox::EventId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_methodology_outbox_folder_status_spec_created_event_id")
                    .table(MethodologyPhaseEventOutbox::Table)
                    .col(MethodologyPhaseEventOutbox::SpecFolder)
                    .col(MethodologyPhaseEventOutbox::Status)
                    .col(MethodologyPhaseEventOutbox::SpecId)
                    .col(MethodologyPhaseEventOutbox::CreatedAt)
                    .col(MethodologyPhaseEventOutbox::EventId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_outbox_spec_status_created_event_id")
                    .table(MethodologyPhaseEventOutbox::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_outbox_folder_status_spec_created_event_id")
                    .table(MethodologyPhaseEventOutbox::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum MethodologyPhaseEventOutbox {
    Table,
    EventId,
    SpecId,
    SpecFolder,
    Status,
    CreatedAt,
}
