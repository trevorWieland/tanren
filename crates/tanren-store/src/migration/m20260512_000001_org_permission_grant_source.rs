//! Add `grant_source` column to `organization_permission_grants` and a
//! pagination-optimized index on `memberships (org_id, id)`.
//!
//! The column is added as `NOT NULL DEFAULT 'direct'` so no separate
//! backfill step is needed — existing rows receive the default on
//! access, and SQLite-compatible (no MODIFY COLUMN).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_grant_source_column(manager).await?;
        add_memberships_org_id_id_index(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_memberships_org_id_id_index(manager).await?;
        drop_grant_source_column(manager).await
    }
}

async fn add_grant_source_column(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(OrganizationPermissionGrants::Table)
                .add_column(
                    ColumnDef::new(OrganizationPermissionGrants::GrantSource)
                        .string()
                        .not_null()
                        .default("direct"),
                )
                .to_owned(),
        )
        .await
}

async fn add_memberships_org_id_id_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_memberships_org_id_id")
                .table(Memberships::Table)
                .col(Memberships::OrgId)
                .col(Memberships::Id)
                .to_owned(),
        )
        .await
}

async fn drop_memberships_org_id_id_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_memberships_org_id_id")
                .table(Memberships::Table)
                .to_owned(),
        )
        .await
}

async fn drop_grant_source_column(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(OrganizationPermissionGrants::Table)
                .drop_column(OrganizationPermissionGrants::GrantSource)
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum OrganizationPermissionGrants {
    Table,
    GrantSource,
}

#[derive(DeriveIden)]
enum Memberships {
    Table,
    OrgId,
    Id,
}
