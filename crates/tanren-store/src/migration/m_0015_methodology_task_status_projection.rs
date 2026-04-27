use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0015_methodology_task_status_projection"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(MethodologyTaskStatus::Table)
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::TaskId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::SpecId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::Status)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::GateChecked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::Audited)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::Adherent)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::ExtraGuards)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskStatus::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_methodology_task_status_spec")
                    .table(MethodologyTaskStatus::Table)
                    .col(MethodologyTaskStatus::SpecId)
                    .col(MethodologyTaskStatus::UpdatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_task_status_spec")
                    .table(MethodologyTaskStatus::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
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
    Status,
    GateChecked,
    Audited,
    Adherent,
    ExtraGuards,
    UpdatedAt,
}
