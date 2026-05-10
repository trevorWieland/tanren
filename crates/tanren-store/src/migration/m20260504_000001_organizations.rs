//! R-0002 migration: add organization rows and organization-level permission
//! grants used by create/list/check organization flows.

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
        create_organizations_table(manager).await?;
        create_organizations_name_index(manager).await?;
        create_memberships_account_lookup_index(manager).await?;
        create_permission_grants_table(manager).await?;
        create_permission_grants_indexes(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_permission_grants_indexes(manager).await?;
        drop_permission_grants_table(manager).await?;
        drop_memberships_account_lookup_index(manager).await?;
        drop_organizations_name_index(manager).await?;
        drop_organizations_table(manager).await?;
        Ok(())
    }
}

async fn create_organizations_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut table = Table::create();
    let mut fk_created_by = ForeignKey::create();
    fk_created_by
        .name("fk_organizations_created_by_account")
        .from(Organizations::Table, Organizations::CreatedByAccountId)
        .to(Accounts::Table, Accounts::Id)
        .on_delete(ForeignKeyAction::Restrict)
        .on_update(ForeignKeyAction::Cascade);
    table
        .table(Organizations::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(Organizations::Id)
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(Organizations::Name).string().not_null())
        .col(
            ColumnDef::new(Organizations::CreatedByAccountId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(Organizations::CreatedAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .foreign_key(&mut fk_created_by);

    manager.create_table(table.clone()).await
}

async fn create_organizations_name_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_organizations_name_unique")
                .table(Organizations::Table)
                .col(Organizations::Name)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_memberships_account_lookup_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_memberships_account_lookup")
                .table(Memberships::Table)
                .col(Memberships::AccountId)
                .to_owned(),
        )
        .await
}

async fn create_permission_grants_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut table = Table::create();
    let mut fk_org = ForeignKey::create();
    fk_org
        .name("fk_org_permission_grants_org")
        .from(
            OrganizationPermissionGrants::Table,
            OrganizationPermissionGrants::OrgId,
        )
        .to(Organizations::Table, Organizations::Id)
        .on_delete(ForeignKeyAction::Cascade)
        .on_update(ForeignKeyAction::Cascade);
    let mut fk_account = ForeignKey::create();
    fk_account
        .name("fk_org_permission_grants_account")
        .from(
            OrganizationPermissionGrants::Table,
            OrganizationPermissionGrants::AccountId,
        )
        .to(Accounts::Table, Accounts::Id)
        .on_delete(ForeignKeyAction::Restrict)
        .on_update(ForeignKeyAction::Cascade);
    let mut fk_granted_by_account = ForeignKey::create();
    fk_granted_by_account
        .name("fk_org_permission_grants_granted_by_account")
        .from(
            OrganizationPermissionGrants::Table,
            OrganizationPermissionGrants::GrantedByAccountId,
        )
        .to(Accounts::Table, Accounts::Id)
        .on_delete(ForeignKeyAction::Restrict)
        .on_update(ForeignKeyAction::Cascade);

    table
        .table(OrganizationPermissionGrants::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(OrganizationPermissionGrants::Id)
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(OrganizationPermissionGrants::OrgId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationPermissionGrants::AccountId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationPermissionGrants::Permission)
                .string()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationPermissionGrants::GrantedByAccountId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(OrganizationPermissionGrants::CreatedAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .foreign_key(&mut fk_org)
        .foreign_key(&mut fk_account)
        .foreign_key(&mut fk_granted_by_account);

    manager.create_table(table.clone()).await
}

async fn create_permission_grants_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_org_permission_grants_holder_count")
                .table(OrganizationPermissionGrants::Table)
                .col(OrganizationPermissionGrants::OrgId)
                .col(OrganizationPermissionGrants::Permission)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_org_permission_grants_permission_lookup")
                .table(OrganizationPermissionGrants::Table)
                .col(OrganizationPermissionGrants::OrgId)
                .col(OrganizationPermissionGrants::AccountId)
                .col(OrganizationPermissionGrants::Permission)
                .unique()
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_org_permission_grants_holder_count")
                .table(OrganizationPermissionGrants::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_index(
            Index::drop()
                .name("idx_org_permission_grants_permission_lookup")
                .table(OrganizationPermissionGrants::Table)
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(
            Table::drop()
                .table(OrganizationPermissionGrants::Table)
                .to_owned(),
        )
        .await
}

async fn drop_memberships_account_lookup_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_memberships_account_lookup")
                .table(Memberships::Table)
                .to_owned(),
        )
        .await
}

async fn drop_organizations_name_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_organizations_name_unique")
                .table(Organizations::Table)
                .to_owned(),
        )
        .await
}

async fn drop_organizations_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(Organizations::Table).to_owned())
        .await
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
    Name,
    CreatedByAccountId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Memberships {
    Table,
    AccountId,
}

#[derive(DeriveIden)]
enum OrganizationPermissionGrants {
    Table,
    Id,
    OrgId,
    AccountId,
    Permission,
    GrantedByAccountId,
    CreatedAt,
}
