use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0019_methodology_idempotency_reservation_lease"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_column("methodology_idempotency", "reservation_expires_at")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(MethodologyIdempotency::Table)
                        .add_column(
                            ColumnDef::new(MethodologyIdempotency::ReservationExpiresAt)
                                .timestamp_with_time_zone()
                                .null()
                                .to_owned(),
                        )
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_methodology_idempotency_reservation_expiry")
                    .table(MethodologyIdempotency::Table)
                    .col(MethodologyIdempotency::ReservationExpiresAt)
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
    ReservationExpiresAt,
}
