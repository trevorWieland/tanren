//! R-0001 migration: create the `accounts`, `memberships`,
//! `invitations`, and `account_sessions` tables.

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
                    .table(Accounts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Accounts::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Accounts::Identifier)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Accounts::DisplayName).string().not_null())
                    .col(ColumnDef::new(Accounts::PasswordHash).binary().not_null())
                    .col(ColumnDef::new(Accounts::PasswordSalt).binary().not_null())
                    .col(
                        ColumnDef::new(Accounts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Accounts::OrgId).uuid())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Memberships::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Memberships::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Memberships::AccountId).uuid().not_null())
                    .col(ColumnDef::new(Memberships::OrgId).uuid().not_null())
                    .col(
                        ColumnDef::new(Memberships::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_memberships_account_org_unique")
                    .table(Memberships::Table)
                    .col(Memberships::AccountId)
                    .col(Memberships::OrgId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Invitations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Invitations::Token)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Invitations::InvitingOrgId).uuid().not_null())
                    .col(
                        ColumnDef::new(Invitations::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Invitations::ConsumedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(AccountSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AccountSessions::Token)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AccountSessions::AccountId).uuid().not_null())
                    .col(
                        ColumnDef::new(AccountSessions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AccountSessions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Invitations::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_memberships_account_org_unique")
                    .table(Memberships::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Memberships::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Accounts::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
    Identifier,
    DisplayName,
    PasswordHash,
    PasswordSalt,
    CreatedAt,
    OrgId,
}

#[derive(DeriveIden)]
enum Memberships {
    Table,
    Id,
    AccountId,
    OrgId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Invitations {
    Table,
    Token,
    InvitingOrgId,
    ExpiresAt,
    ConsumedAt,
}

#[derive(DeriveIden)]
enum AccountSessions {
    Table,
    Token,
    AccountId,
    CreatedAt,
}
