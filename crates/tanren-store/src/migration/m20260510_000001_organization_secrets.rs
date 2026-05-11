//! R-0011 migration: add organization-secret metadata and encrypted value-version
//! tables.
//!
//! Two tables:
//!
//! - `organization_secrets`: metadata rows keyed by `(org_id, name)` with
//!   lifecycle status, baseline use policy, current version, and timestamps.
//!   The unique composite index `idx_org_secrets_org_name` ensures per-org
//!   name uniqueness.
//!
//! - `organization_secret_values`: encrypted value versions keyed by
//!   `(secret_id, version)`. Each row stores AES-256-GCM ciphertext, the
//!   random nonce used for encryption, and the key-id that identifies which
//!   installation-managed key produced the ciphertext. Rows are never updated
//!   — a new version always inserts a fresh row.
//!
//! Indexes support:
//! - `idx_org_secrets_org_name`: unique org/name lookup (create/duplicate check).
//! - `idx_org_secrets_org_status`: bounded org-scoped list with status filter.
//! - `idx_org_secrets_org_id_cursor`: cursor-based pagination by secret id.
//! - `idx_org_secret_values_active_version`: direct active-version lookup by
//!   `secret_id` + version without scanning unrelated secrets.

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
        create_organization_secrets_table(manager).await?;
        create_organization_secrets_indexes(manager).await?;
        create_organization_secret_values_table(manager).await?;
        create_organization_secret_values_indexes(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_organization_secret_values_indexes(manager).await?;
        drop_organization_secret_values_table(manager).await?;
        drop_organization_secrets_indexes(manager).await?;
        drop_organization_secrets_table(manager).await?;
        Ok(())
    }
}

async fn create_organization_secrets_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut table = Table::create();
    let mut fk_org = ForeignKey::create();
    fk_org
        .name("fk_org_secrets_org")
        .from(OrganizationSecrets::Table, OrganizationSecrets::OrgId)
        .to(Organizations::Table, Organizations::Id)
        .on_delete(ForeignKeyAction::Cascade)
        .on_update(ForeignKeyAction::Cascade);
    let mut fk_created_by = ForeignKey::create();
    fk_created_by
        .name("fk_org_secrets_created_by")
        .from(
            OrganizationSecrets::Table,
            OrganizationSecrets::CreatedByAccountId,
        )
        .to(Accounts::Table, Accounts::Id)
        .on_delete(ForeignKeyAction::Restrict)
        .on_update(ForeignKeyAction::Cascade);
    table
        .table(OrganizationSecrets::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(OrganizationSecrets::Id)
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(OrganizationSecrets::OrgId).uuid().not_null())
        .col(
            ColumnDef::new(OrganizationSecrets::Name)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecrets::OwnerScope)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecrets::Status)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecrets::UsePolicy)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecrets::Version)
                .integer()
                .not_null(),
        )
        .col(ColumnDef::new(OrganizationSecrets::Description).string())
        .col(ColumnDef::new(OrganizationSecrets::Provider).string())
        .col(
            ColumnDef::new(OrganizationSecrets::CreatedByAccountId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecrets::CreatedAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecrets::UpdatedAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(ColumnDef::new(OrganizationSecrets::DeletedAt).timestamp_with_time_zone())
        .foreign_key(&mut fk_org)
        .foreign_key(&mut fk_created_by);

    manager.create_table(table.clone()).await
}

async fn create_organization_secrets_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_org_secrets_org_name")
                .table(OrganizationSecrets::Table)
                .col(OrganizationSecrets::OrgId)
                .col(OrganizationSecrets::Name)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_org_secrets_org_status")
                .table(OrganizationSecrets::Table)
                .col(OrganizationSecrets::OrgId)
                .col(OrganizationSecrets::Status)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_org_secrets_org_id_cursor")
                .table(OrganizationSecrets::Table)
                .col(OrganizationSecrets::OrgId)
                .col(OrganizationSecrets::Id)
                .to_owned(),
        )
        .await
}

async fn drop_organization_secrets_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_org_secrets_org_id_cursor")
                .table(OrganizationSecrets::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_index(
            Index::drop()
                .name("idx_org_secrets_org_status")
                .table(OrganizationSecrets::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_index(
            Index::drop()
                .name("idx_org_secrets_org_name")
                .table(OrganizationSecrets::Table)
                .to_owned(),
        )
        .await
}

async fn drop_organization_secrets_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(OrganizationSecrets::Table).to_owned())
        .await
}

async fn create_organization_secret_values_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut table = Table::create();
    let mut fk_secret = ForeignKey::create();
    fk_secret
        .name("fk_org_secret_values_secret")
        .from(
            OrganizationSecretValues::Table,
            OrganizationSecretValues::SecretId,
        )
        .to(OrganizationSecrets::Table, OrganizationSecrets::Id)
        .on_delete(ForeignKeyAction::Cascade)
        .on_update(ForeignKeyAction::Cascade);
    table
        .table(OrganizationSecretValues::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(OrganizationSecretValues::SecretId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecretValues::Version)
                .integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecretValues::EncryptedValue)
                .binary()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecretValues::Nonce)
                .binary()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecretValues::KeyId)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationSecretValues::CreatedAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .primary_key(
            Index::create()
                .col(OrganizationSecretValues::SecretId)
                .col(OrganizationSecretValues::Version),
        )
        .foreign_key(&mut fk_secret);

    manager.create_table(table.clone()).await
}

async fn create_organization_secret_values_indexes(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_org_secret_values_active_version")
                .table(OrganizationSecretValues::Table)
                .col(OrganizationSecretValues::SecretId)
                .col(OrganizationSecretValues::Version)
                .to_owned(),
        )
        .await
}

async fn drop_organization_secret_values_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_org_secret_values_active_version")
                .table(OrganizationSecretValues::Table)
                .to_owned(),
        )
        .await
}

async fn drop_organization_secret_values_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(
            Table::drop()
                .table(OrganizationSecretValues::Table)
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum OrganizationSecrets {
    Table,
    Id,
    OrgId,
    Name,
    OwnerScope,
    Status,
    UsePolicy,
    Version,
    Description,
    Provider,
    CreatedByAccountId,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(DeriveIden)]
enum OrganizationSecretValues {
    Table,
    SecretId,
    Version,
    EncryptedValue,
    Nonce,
    KeyId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}
