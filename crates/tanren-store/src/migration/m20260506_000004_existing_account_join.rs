//! R-0006 migration: extend invitations and memberships for
//! existing-account join, org-level permissions, and revoked/consumed
//! attribution.

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
                    .add_column(ColumnDef::new(Invitations::TargetIdentifier).text().null())
                    .add_column(ColumnDef::new(Invitations::OrgPermissions).text().null())
                    .add_column(
                        ColumnDef::new(Invitations::RevokedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .add_column(ColumnDef::new(Invitations::RevokedBy).uuid().null())
                    .add_column(ColumnDef::new(Invitations::ConsumedBy).uuid().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Memberships::Table)
                    .add_column(ColumnDef::new(Memberships::OrgPermissions).text().null())
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
                    .drop_column(Memberships::OrgPermissions)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Invitations::Table)
                    .drop_column(Invitations::ConsumedBy)
                    .drop_column(Invitations::RevokedBy)
                    .drop_column(Invitations::RevokedAt)
                    .drop_column(Invitations::OrgPermissions)
                    .drop_column(Invitations::TargetIdentifier)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Invitations {
    Table,
    TargetIdentifier,
    OrgPermissions,
    RevokedAt,
    RevokedBy,
    ConsumedBy,
}

#[derive(DeriveIden)]
enum Memberships {
    Table,
    OrgPermissions,
}
