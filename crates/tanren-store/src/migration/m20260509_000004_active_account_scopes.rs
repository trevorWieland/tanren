//! R-0003 sub-PR migration: add durable active-account scope projection.
//!
//! The `active_account_scopes` table stores the latest active-account id
//! per signed-in account-id scope key. Switch handling updates this row
//! and appends `active_account_switched` inside one transaction so
//! success-path state and event durability are atomic.

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
                    .table(ActiveAccountScopes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ActiveAccountScopes::ScopeHash)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ActiveAccountScopes::ActiveAccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActiveAccountScopes::SignedInAccountIds)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActiveAccountScopes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ActiveAccountScopes::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ActiveAccountScopes {
    Table,
    ScopeHash,
    ActiveAccountId,
    SignedInAccountIds,
    UpdatedAt,
}
