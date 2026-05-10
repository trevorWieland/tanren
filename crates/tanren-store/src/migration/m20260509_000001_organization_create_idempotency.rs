//! Add organization-create idempotency records.
//!
//! Retention policy:
//! - keep rows for 30 days from `created_at`;
//! - prune stale rows with:
//!   `DELETE FROM organization_create_idempotency WHERE created_at < now() - interval '30 days'`;
//! - use `idx_org_create_idempotency_created_at` to keep stale-row scans bounded.

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
        let mut table = Table::create();
        let mut fk_account = ForeignKey::create();
        fk_account
            .name("fk_org_create_idempotency_account")
            .from(
                OrganizationCreateIdempotency::Table,
                OrganizationCreateIdempotency::AccountId,
            )
            .to(Accounts::Table, Accounts::Id)
            .on_delete(ForeignKeyAction::Cascade)
            .on_update(ForeignKeyAction::Cascade);
        let mut fk_organization = ForeignKey::create();
        fk_organization
            .name("fk_org_create_idempotency_org")
            .from(
                OrganizationCreateIdempotency::Table,
                OrganizationCreateIdempotency::OrganizationId,
            )
            .to(Organizations::Table, Organizations::Id)
            .on_delete(ForeignKeyAction::Cascade)
            .on_update(ForeignKeyAction::Cascade);
        table
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
                ColumnDef::new(OrganizationCreateIdempotency::RequestFingerprint)
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
            .foreign_key(&mut fk_account)
            .foreign_key(&mut fk_organization);

        manager.create_table(table.clone()).await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_org_create_idempotency_created_at")
                    .table(OrganizationCreateIdempotency::Table)
                    .col(OrganizationCreateIdempotency::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_org_create_idempotency_created_at")
                    .table(OrganizationCreateIdempotency::Table)
                    .to_owned(),
            )
            .await?;
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
    RequestFingerprint,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}
