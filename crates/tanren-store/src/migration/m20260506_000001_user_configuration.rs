//! R-0008 migration: create `user_config_values` and `user_credentials`
//! tables.

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
                    .table(UserConfigValues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserConfigValues::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserConfigValues::AccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserConfigValues::Key).string().not_null())
                    .col(ColumnDef::new(UserConfigValues::Value).text().not_null())
                    .col(
                        ColumnDef::new(UserConfigValues::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_config_values_account_key_unique")
                    .table(UserConfigValues::Table)
                    .col(UserConfigValues::AccountId)
                    .col(UserConfigValues::Key)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserCredentials::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserCredentials::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserCredentials::AccountId).uuid().not_null())
                    .col(ColumnDef::new(UserCredentials::Kind).string().not_null())
                    .col(ColumnDef::new(UserCredentials::Scope).string().not_null())
                    .col(ColumnDef::new(UserCredentials::Name).string().not_null())
                    .col(ColumnDef::new(UserCredentials::Description).text())
                    .col(ColumnDef::new(UserCredentials::Provider).string())
                    .col(
                        ColumnDef::new(UserCredentials::EncryptedValue)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentials::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserCredentials::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_credentials_account_kind_name_unique")
                    .table(UserCredentials::Table)
                    .col(UserCredentials::AccountId)
                    .col(UserCredentials::Kind)
                    .col(UserCredentials::Name)
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
                    .name("idx_user_credentials_account_kind_name_unique")
                    .table(UserCredentials::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserCredentials::Table).to_owned())
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_config_values_account_key_unique")
                    .table(UserConfigValues::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserConfigValues::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum UserConfigValues {
    Table,
    Id,
    AccountId,
    Key,
    Value,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserCredentials {
    Table,
    Id,
    AccountId,
    Kind,
    Scope,
    Name,
    Description,
    Provider,
    EncryptedValue,
    CreatedAt,
    UpdatedAt,
}
