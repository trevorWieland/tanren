//! R-0041 migration: persist role templates, role-permission bundles, and
//! direct permission grants.
//!
//! `permission_grants.source_ref` intentionally has no foreign-key
//! constraints to source tables: deleting a role template must never delete
//! historical grants. Grant source metadata is audit-only.

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
        create_roles_table(manager).await?;
        create_role_permissions_table(manager).await?;
        create_permission_grants_table(manager).await?;
        create_roles_scope_name_index(manager).await?;
        create_role_permissions_role_id_index(manager).await?;
        create_permission_grants_dedup_index(manager).await?;
        create_permission_grants_lookup_index(manager).await?;
        create_permission_grants_grantee_permission_index(manager).await?;
        create_permission_grants_grantee_page_index(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_permission_grants_grantee_page_index(manager).await?;
        drop_permission_grants_grantee_permission_index(manager).await?;
        drop_permission_grants_lookup_index(manager).await?;
        drop_permission_grants_dedup_index(manager).await?;
        drop_permission_grants_table(manager).await?;
        drop_role_permissions_role_id_index(manager).await?;
        drop_role_permissions_table(manager).await?;
        drop_roles_scope_name_index(manager).await?;
        drop_roles_table(manager).await?;
        Ok(())
    }
}

async fn create_roles_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Roles::Table)
                .if_not_exists()
                .col(ColumnDef::new(Roles::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Roles::ScopeKind).string().not_null())
                .col(ColumnDef::new(Roles::ScopeRef).uuid().not_null())
                .col(ColumnDef::new(Roles::Name).string().not_null())
                .col(
                    ColumnDef::new(Roles::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Roles::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .to_owned(),
        )
        .await
}

async fn create_role_permissions_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(RolePermissions::Table)
                .if_not_exists()
                .col(ColumnDef::new(RolePermissions::RoleId).uuid().not_null())
                .col(
                    ColumnDef::new(RolePermissions::PermissionName)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RolePermissions::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_role_permissions_role_id")
                        .from(RolePermissions::Table, RolePermissions::RoleId)
                        .to(Roles::Table, Roles::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .primary_key(
                    Index::create()
                        .col(RolePermissions::RoleId)
                        .col(RolePermissions::PermissionName),
                )
                .to_owned(),
        )
        .await
}

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
                    ColumnDef::new(PermissionGrants::GranteeKind)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(PermissionGrants::GranteeRef)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(PermissionGrants::ScopeKind)
                        .string()
                        .not_null(),
                )
                .col(ColumnDef::new(PermissionGrants::ScopeRef).uuid().not_null())
                .col(
                    ColumnDef::new(PermissionGrants::PermissionName)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(PermissionGrants::SourceKind)
                        .string()
                        .not_null(),
                )
                .col(ColumnDef::new(PermissionGrants::SourceRef).uuid().null())
                .col(
                    ColumnDef::new(PermissionGrants::GrantedByKind)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(PermissionGrants::GrantedByRef)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(PermissionGrants::GrantedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(PermissionGrants::RevokedByKind)
                        .string()
                        .null(),
                )
                .col(ColumnDef::new(PermissionGrants::RevokedByRef).uuid().null())
                .col(
                    ColumnDef::new(PermissionGrants::RevokedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .to_owned(),
        )
        .await
}

async fn create_roles_scope_name_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_roles_scope_name")
                .table(Roles::Table)
                .col(Roles::ScopeKind)
                .col(Roles::ScopeRef)
                .col(Roles::Name)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_role_permissions_role_id_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_role_permissions_role_id")
                .table(RolePermissions::Table)
                .col(RolePermissions::RoleId)
                .to_owned(),
        )
        .await
}

async fn create_permission_grants_dedup_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_permission_grants_dedup")
                .table(PermissionGrants::Table)
                .col(PermissionGrants::GranteeKind)
                .col(PermissionGrants::GranteeRef)
                .col(PermissionGrants::ScopeKind)
                .col(PermissionGrants::ScopeRef)
                .col(PermissionGrants::PermissionName)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_permission_grants_lookup_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_permission_grants_grantee_scope_permission")
                .table(PermissionGrants::Table)
                .col(PermissionGrants::GranteeKind)
                .col(PermissionGrants::GranteeRef)
                .col(PermissionGrants::ScopeKind)
                .col(PermissionGrants::ScopeRef)
                .col(PermissionGrants::PermissionName)
                .col(PermissionGrants::RevokedAt)
                .to_owned(),
        )
        .await
}

async fn create_permission_grants_grantee_permission_index(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_permission_grants_grantee_permission")
                .table(PermissionGrants::Table)
                .col(PermissionGrants::GranteeKind)
                .col(PermissionGrants::GranteeRef)
                .col(PermissionGrants::PermissionName)
                .col(PermissionGrants::RevokedAt)
                .to_owned(),
        )
        .await
}

async fn create_permission_grants_grantee_page_index(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_permission_grants_grantee_page")
                .table(PermissionGrants::Table)
                .col(PermissionGrants::GranteeKind)
                .col(PermissionGrants::GranteeRef)
                .col(PermissionGrants::RevokedAt)
                .col(PermissionGrants::GrantedAt)
                .col(PermissionGrants::Id)
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_lookup_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_permission_grants_grantee_scope_permission")
                .table(PermissionGrants::Table)
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_grantee_permission_index(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_permission_grants_grantee_permission")
                .table(PermissionGrants::Table)
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_grantee_page_index(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_permission_grants_grantee_page")
                .table(PermissionGrants::Table)
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_dedup_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_permission_grants_dedup")
                .table(PermissionGrants::Table)
                .to_owned(),
        )
        .await
}

async fn drop_permission_grants_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(PermissionGrants::Table).to_owned())
        .await
}

async fn drop_role_permissions_role_id_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_role_permissions_role_id")
                .table(RolePermissions::Table)
                .to_owned(),
        )
        .await
}

async fn drop_role_permissions_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(RolePermissions::Table).to_owned())
        .await
}

async fn drop_roles_scope_name_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_roles_scope_name")
                .table(Roles::Table)
                .to_owned(),
        )
        .await
}

async fn drop_roles_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(Roles::Table).to_owned())
        .await
}

#[derive(DeriveIden)]
enum Roles {
    Table,
    Id,
    ScopeKind,
    ScopeRef,
    Name,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RolePermissions {
    Table,
    RoleId,
    PermissionName,
    CreatedAt,
}

#[derive(DeriveIden)]
enum PermissionGrants {
    Table,
    Id,
    GranteeKind,
    GranteeRef,
    ScopeKind,
    ScopeRef,
    PermissionName,
    SourceKind,
    SourceRef,
    GrantedByKind,
    GrantedByRef,
    GrantedAt,
    RevokedByKind,
    RevokedByRef,
    RevokedAt,
}
