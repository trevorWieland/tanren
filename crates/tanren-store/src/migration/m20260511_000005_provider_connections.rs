//! Provider connections projection table and reachable-repos projection.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_provider_connections(manager).await?;
        create_account_provider_index(manager).await?;
        create_reachable_repos(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ProviderConnectionReachableRepos::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ProviderConnections::Table).to_owned())
            .await
    }
}

async fn create_provider_connections(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProviderConnections::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ProviderConnections::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::AccountId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::ProviderKind)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::ProviderName)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::Status)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::OpaqueAccessToken)
                        .text()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::ExternalAccountId)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnections::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .check(Expr::col(ProviderConnections::Status).is_in([
                    "initiated",
                    "established",
                    "failed",
                ]))
                .check(
                    Expr::col(ProviderConnections::ProviderKind)
                        .is_in(["source_control", "identity"]),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_provider_connections_account_id_accounts")
                        .from(ProviderConnections::Table, ProviderConnections::AccountId)
                        .to(Accounts::Table, Accounts::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
}

async fn create_account_provider_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_provider_connections_account_provider")
                .table(ProviderConnections::Table)
                .col(ProviderConnections::AccountId)
                .col(ProviderConnections::ProviderKind)
                .to_owned(),
        )
        .await
}

async fn create_reachable_repos(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProviderConnectionReachableRepos::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ProviderConnectionReachableRepos::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(ProviderConnectionReachableRepos::ConnectionId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnectionReachableRepos::RepositoryFullName)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnectionReachableRepos::RepositoryUrl)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderConnectionReachableRepos::DiscoveredAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_reachable_repos_connection_id")
                        .from(
                            ProviderConnectionReachableRepos::Table,
                            ProviderConnectionReachableRepos::ConnectionId,
                        )
                        .to(ProviderConnections::Table, ProviderConnections::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum ProviderConnections {
    Table,
    Id,
    AccountId,
    ProviderKind,
    ProviderName,
    Status,
    OpaqueAccessToken,
    ExternalAccountId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ProviderConnectionReachableRepos {
    Table,
    Id,
    ConnectionId,
    RepositoryFullName,
    RepositoryUrl,
    DiscoveredAt,
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}
