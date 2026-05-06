//! R-0002 migration: create the `organizations` and
//! `organization_permission_grants` tables.

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
                    .table(Organizations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Organizations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Organizations::CanonicalName)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Organizations::DisplayName).text().not_null())
                    .col(
                        ColumnDef::new(Organizations::CreatorAccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Organizations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_organizations_canonical_name_per_creator")
                    .table(Organizations::Table)
                    .col(Organizations::CanonicalName)
                    .col(Organizations::CreatorAccountId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
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
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrganizationPermissionGrants::GrantedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_org_permission_grants_unique")
                    .table(OrganizationPermissionGrants::Table)
                    .col(OrganizationPermissionGrants::OrgId)
                    .col(OrganizationPermissionGrants::AccountId)
                    .col(OrganizationPermissionGrants::Permission)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_org_permission_grants_org_permission")
                    .table(OrganizationPermissionGrants::Table)
                    .col(OrganizationPermissionGrants::OrgId)
                    .col(OrganizationPermissionGrants::Permission)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_org_permission_grants_org_permission")
                    .table(OrganizationPermissionGrants::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_org_permission_grants_unique")
                    .table(OrganizationPermissionGrants::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(OrganizationPermissionGrants::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_organizations_canonical_name_per_creator")
                    .table(Organizations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Organizations::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
    CanonicalName,
    DisplayName,
    CreatorAccountId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum OrganizationPermissionGrants {
    Table,
    Id,
    OrgId,
    AccountId,
    Permission,
    GrantedAt,
}
