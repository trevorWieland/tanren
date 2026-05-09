//! Add organization-create idempotency records.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrganizationCreateIdempotency::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::AccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::IdempotencyKey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::OrganizationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::OrganizationName)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(OrganizationCreateIdempotency::AccountId)
                            .col(OrganizationCreateIdempotency::IdempotencyKey),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(OrganizationCreateIdempotency::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum OrganizationCreateIdempotency {
    Table,
    AccountId,
    IdempotencyKey,
    OrganizationId,
    OrganizationName,
    CreatedAt,
}
