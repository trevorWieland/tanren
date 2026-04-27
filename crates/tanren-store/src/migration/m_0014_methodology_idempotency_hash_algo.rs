use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0014_methodology_idempotency_hash_algo"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_column("methodology_idempotency", "request_hash_algo")
            .await?
        {
            return Ok(());
        }
        manager
            .alter_table(
                Table::alter()
                    .table(MethodologyIdempotency::Table)
                    .add_column(
                        ColumnDef::new(MethodologyIdempotency::RequestHashAlgo)
                            .string()
                            .not_null()
                            .default("default-hasher-json-v0")
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Column drops are intentionally skipped for SQLite portability.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum MethodologyIdempotency {
    Table,
    RequestHashAlgo,
}
