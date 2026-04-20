use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0020_methodology_task_finding_projection"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(MethodologyTaskFinding::Table)
                    .col(
                        ColumnDef::new(MethodologyTaskFinding::TaskId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskFinding::FindingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskFinding::SpecId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskFinding::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(MethodologyTaskFinding::TaskId)
                            .col(MethodologyTaskFinding::FindingId),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_methodology_task_finding_spec_task")
                    .table(MethodologyTaskFinding::Table)
                    .col(MethodologyTaskFinding::SpecId)
                    .col(MethodologyTaskFinding::TaskId)
                    .col(MethodologyTaskFinding::UpdatedAt)
                    .col(MethodologyTaskFinding::FindingId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_task_finding_spec_task")
                    .table(MethodologyTaskFinding::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MethodologyTaskFinding::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum MethodologyTaskFinding {
    Table,
    TaskId,
    FindingId,
    SpecId,
    UpdatedAt,
}
