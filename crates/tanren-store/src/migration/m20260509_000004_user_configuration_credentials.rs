//! R-0008 migration: user-tier configuration values plus credential metadata and
//! encrypted credential value storage.

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
        self.create_user_config_values(manager).await?;
        self.create_user_credentials(manager).await?;
        self.create_user_credential_values(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        self.drop_user_credential_values(manager).await?;
        self.drop_user_credentials(manager).await?;
        self.drop_user_config_values(manager).await?;
        Ok(())
    }
}

impl Migration {
    async fn create_user_config_values(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
                    .col(
                        ColumnDef::new(UserConfigValues::OwnerScope)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserConfigValues::Key).string().not_null())
                    .col(
                        ColumnDef::new(UserConfigValues::ValueKind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserConfigValues::ValueJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserConfigValues::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
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
            .create_index(
                Index::create()
                    .name("idx_user_config_values_account_updated_key")
                    .table(UserConfigValues::Table)
                    .col(UserConfigValues::AccountId)
                    .col(UserConfigValues::UpdatedAt)
                    .col(UserConfigValues::Key)
                    .to_owned(),
            )
            .await
    }

    async fn create_user_credentials(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
                    .col(
                        ColumnDef::new(UserCredentials::OwnerScope)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserCredentials::Kind).string().not_null())
                    .col(ColumnDef::new(UserCredentials::Status).string().not_null())
                    .col(
                        ColumnDef::new(UserCredentials::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentials::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_user_credentials_account_scope_kind")
                    .table(UserCredentials::Table)
                    .col(UserCredentials::AccountId)
                    .col(UserCredentials::OwnerScope)
                    .col(UserCredentials::Kind)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_user_credentials_scope_updated_id")
                    .table(UserCredentials::Table)
                    .col(UserCredentials::AccountId)
                    .col(UserCredentials::OwnerScope)
                    .col(UserCredentials::UpdatedAt)
                    .col(UserCredentials::Id)
                    .to_owned(),
            )
            .await
    }

    async fn create_user_credential_values(
        &self,
        manager: &SchemaManager<'_>,
    ) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserCredentialValues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserCredentialValues::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::ItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::AccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::CipherScheme)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::KdfVersion)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::KdfSalt)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::Nonce)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::Ciphertext)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserCredentialValues::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_user_credential_values_item_unique")
                    .table(UserCredentialValues::Table)
                    .col(UserCredentialValues::ItemId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_user_credential_values_account_item")
                    .table(UserCredentialValues::Table)
                    .col(UserCredentialValues::AccountId)
                    .col(UserCredentialValues::ItemId)
                    .to_owned(),
            )
            .await
    }

    async fn drop_user_config_values(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_config_values_account_updated_key")
                    .table(UserConfigValues::Table)
                    .to_owned(),
            )
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
            .await
    }

    async fn drop_user_credentials(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_credentials_scope_updated_id")
                    .table(UserCredentials::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_credentials_account_scope_kind")
                    .table(UserCredentials::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserCredentials::Table).to_owned())
            .await
    }

    async fn drop_user_credential_values(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_credential_values_account_item")
                    .table(UserCredentialValues::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_user_credential_values_item_unique")
                    .table(UserCredentialValues::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(UserCredentialValues::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserConfigValues {
    Table,
    Id,
    AccountId,
    OwnerScope,
    Key,
    ValueKind,
    ValueJson,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserCredentials {
    Table,
    Id,
    AccountId,
    OwnerScope,
    Kind,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserCredentialValues {
    Table,
    Id,
    ItemId,
    AccountId,
    CipherScheme,
    KdfVersion,
    KdfSalt,
    Nonce,
    Ciphertext,
    CreatedAt,
    UpdatedAt,
}
