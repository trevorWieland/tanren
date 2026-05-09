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
                    .to_owned(),
            )
            .await?;

        // `OrganizationName` values are persisted in normalized form.
        // A unique index on the stored key guarantees case/whitespace
        // variants cannot create duplicate organizations.
        manager
            .create_index(
                Index::create()
                    .name("idx_organizations_name_unique")
                    .table(Organizations::Table)
                    .col(Organizations::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Hot path for list-organizations by account. The existing
        // `(account_id, org_id)` unique index also helps this query, but we
        // keep a dedicated lookup index name for migration-level clarity.
        manager
            .create_index(
                Index::create()
                    .name("idx_memberships_account_lookup")
                    .table(Memberships::Table)
                    .col(Memberships::AccountId)
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
                    .to_owned(),
            )
            .await?;

        // Permission lookups are by `(org, account, permission)` and each
        // permission can be granted at most once for the pair.
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
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_org_permission_grants_permission_lookup")
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
                    .name("idx_memberships_account_lookup")
                    .table(Memberships::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_organizations_name_unique")
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
    Name,
    CreatedByAccountId,
    CreatedAt,
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
