use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0016_methodology_task_projection_snapshot"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MethodologyTaskStatus::Table)
                    .add_column(
                        ColumnDef::new(MethodologyTaskStatus::TaskJson)
                            .json_binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(MethodologyTaskStatus::Table)
                    .add_column(
                        ColumnDef::new(MethodologyTaskStatus::CreatedAt)
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
                    .name("ix_methodology_task_status_spec_created")
                    .table(MethodologyTaskStatus::Table)
                    .col(MethodologyTaskStatus::SpecId)
                    .col(MethodologyTaskStatus::CreatedAt)
                    .col(MethodologyTaskStatus::TaskId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_task_status_spec_created")
                    .table(MethodologyTaskStatus::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum MethodologyTaskStatus {
    Table,
    TaskId,
    SpecId,
    TaskJson,
    CreatedAt,
}
