use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0017_methodology_spec_lookup_projection"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(MethodologyTaskSpec::Table)
                    .col(
                        ColumnDef::new(MethodologyTaskSpec::TaskId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskSpec::SpecId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologyTaskSpec::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(MethodologySignpostSpec::Table)
                    .col(
                        ColumnDef::new(MethodologySignpostSpec::SignpostId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MethodologySignpostSpec::SpecId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MethodologySignpostSpec::UpdatedAt)
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
                    .name("ix_methodology_task_spec_spec")
                    .table(MethodologyTaskSpec::Table)
                    .col(MethodologyTaskSpec::SpecId)
                    .col(MethodologyTaskSpec::UpdatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_methodology_signpost_spec_spec")
                    .table(MethodologySignpostSpec::Table)
                    .col(MethodologySignpostSpec::SpecId)
                    .col(MethodologySignpostSpec::UpdatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_signpost_spec_spec")
                    .table(MethodologySignpostSpec::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_methodology_task_spec_spec")
                    .table(MethodologyTaskSpec::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MethodologySignpostSpec::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MethodologyTaskSpec::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum MethodologyTaskSpec {
    Table,
    TaskId,
    SpecId,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum MethodologySignpostSpec {
    Table,
    SignpostId,
    SpecId,
    UpdatedAt,
}
