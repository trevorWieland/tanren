//! R-0005 migration: add organization-invitation columns to `invitations`
//! and a `permissions` JSONB column to `memberships`.
//!
//! New columns on `invitations`:
//! - `recipient_identifier` — the invitee's identifier string.
//! - `granted_permissions` — JSONB array of permission name strings.
//! - `created_by_account_id` — the admin who created the invitation.
//! - `created_at` — wall-clock creation time.
//! - `revoked_at` — set when an admin revokes before acceptance.
//!
//! New column on `memberships`:
//! - `permissions` — JSONB array of permission name strings granted to
//!   the member within the organization.

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
            .alter_table(
                Table::alter()
                    .table(Invitations::Table)
                    .add_column(
                        ColumnDef::new(Invitations::RecipientIdentifier)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .add_column(
                        ColumnDef::new(Invitations::GrantedPermissions)
                            .json_binary()
                            .not_null()
                            .default(serde_json::Value::Array(Vec::new())),
                    )
                    .add_column(
                        ColumnDef::new(Invitations::CreatedByAccountId)
                            .uuid()
                            .not_null()
                            .default("00000000-0000-0000-0000-000000000000"),
                    )
                    .add_column(
                        ColumnDef::new(Invitations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .add_column(ColumnDef::new(Invitations::RevokedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Memberships::Table)
                    .add_column(
                        ColumnDef::new(Memberships::Permissions)
                            .json_binary()
                            .not_null()
                            .default(serde_json::Value::Array(Vec::new())),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Memberships::Table)
                    .drop_column(Memberships::Permissions)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Invitations::Table)
                    .drop_column(Invitations::RevokedAt)
                    .drop_column(Invitations::CreatedAt)
                    .drop_column(Invitations::CreatedByAccountId)
                    .drop_column(Invitations::GrantedPermissions)
                    .drop_column(Invitations::RecipientIdentifier)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Invitations {
    Table,
    RecipientIdentifier,
    GrantedPermissions,
    CreatedByAccountId,
    CreatedAt,
    RevokedAt,
}

#[derive(DeriveIden)]
enum Memberships {
    Table,
    Permissions,
}
