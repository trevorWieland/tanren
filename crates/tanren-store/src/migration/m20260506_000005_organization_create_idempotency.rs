//! R-0002 migration: create the `organization_create_idempotency` table.
//!
//! Stores one row per successful organization-creation request keyed by
//! the caller-supplied idempotency key. On retry the store reads the
//! cached response instead of inserting duplicate projection rows or
//! canonical events.

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
                        ColumnDef::new(OrganizationCreateIdempotency::RequestId)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::AccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::CanonicalName)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::ResponseJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationCreateIdempotency::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(OrganizationCreateIdempotency::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum OrganizationCreateIdempotency {
    Table,
    RequestId,
    AccountId,
    CanonicalName,
    ResponseJson,
    CreatedAt,
}
