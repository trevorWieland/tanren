//! R-0040 migration: add normalized tables for self-permission introspection.
//!
//! `permission_grants` stores account-scoped grants at organization or
//! project scope. `permission_constraints` stores optional policy constraints
//! keyed by grant id so effective-state reads can join one constraint per grant.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

impl Migration {
    async fn create_permission_grants_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PermissionGrants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PermissionGrants::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PermissionGrants::AccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PermissionGrants::OrgId).uuid())
                    .col(ColumnDef::new(PermissionGrants::ProjectId).uuid())
                    .col(
                        ColumnDef::new(PermissionGrants::PermissionName)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PermissionGrants::RoleTemplateName).string())
                    .col(
                        ColumnDef::new(PermissionGrants::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn create_permission_grants_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_permission_grants_account_org_scope")
                    .table(PermissionGrants::Table)
                    .col(PermissionGrants::AccountId)
                    .col(PermissionGrants::OrgId)
                    .col(PermissionGrants::PermissionName)
                    .col(PermissionGrants::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_permission_grants_account_project_scope")
                    .table(PermissionGrants::Table)
                    .col(PermissionGrants::AccountId)
                    .col(PermissionGrants::ProjectId)
                    .col(PermissionGrants::PermissionName)
                    .col(PermissionGrants::Id)
                    .to_owned(),
            )
            .await
    }

    async fn create_permission_constraints_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PermissionConstraints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PermissionConstraints::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PermissionConstraints::GrantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionConstraints::Reason)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionConstraints::IsProjectPolicy)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PermissionConstraints::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_permission_constraints_grant_id")
                            .from(PermissionConstraints::Table, PermissionConstraints::GrantId)
                            .to(PermissionGrants::Table, PermissionGrants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn create_permission_constraints_indexes(
        manager: &SchemaManager<'_>,
    ) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_permission_constraints_grant_id_unique")
                    .table(PermissionConstraints::Table)
                    .col(PermissionConstraints::GrantId)
                    .unique()
                    .to_owned(),
            )
            .await
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Self::create_permission_grants_table(manager).await?;
        Self::create_permission_grants_indexes(manager).await?;
        Self::create_permission_constraints_table(manager).await?;
        Self::create_permission_constraints_indexes(manager).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_permission_constraints_grant_id_unique")
                    .table(PermissionConstraints::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(PermissionConstraints::Table).to_owned())
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_permission_grants_account_project_scope")
                    .table(PermissionGrants::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_permission_grants_account_org_scope")
                    .table(PermissionGrants::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(PermissionGrants::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum PermissionGrants {
    Table,
    Id,
    AccountId,
    OrgId,
    ProjectId,
    PermissionName,
    RoleTemplateName,
    CreatedAt,
}

#[derive(DeriveIden)]
enum PermissionConstraints {
    Table,
    Id,
    GrantId,
    Reason,
    IsProjectPolicy,
    CreatedAt,
}
