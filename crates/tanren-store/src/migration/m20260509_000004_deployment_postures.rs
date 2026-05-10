//! Deployment-posture persistence: one current posture per scope.

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
                    .table(DeploymentPostures::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DeploymentPostures::ScopeKind)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeploymentPostures::ScopeId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeploymentPostures::Posture)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeploymentPostures::ChangedBy)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DeploymentPostures::ChangedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_deployment_postures_scope")
                            .col(DeploymentPostures::ScopeKind)
                            .col(DeploymentPostures::ScopeId),
                    )
                    // Closed posture set enforced at the DB layer.
                    .check(Expr::col(DeploymentPostures::Posture).is_in([
                        "hosted",
                        "self_hosted",
                        "local_only",
                    ]))
                    // Closed scope-kind set enforced at the DB layer.
                    .check(Expr::col(DeploymentPostures::ScopeKind).is_in([
                        "account",
                        "project",
                        "installation",
                    ]))
                    // Until project/installation backing tables land, only
                    // account-scoped rows are allowed.
                    .check(Expr::col(DeploymentPostures::ScopeKind).eq("account"))
                    // Every mutation actor must reference an existing account.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_deployment_postures_changed_by_accounts")
                            .from(DeploymentPostures::Table, DeploymentPostures::ChangedBy)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    // Supported account-scoped targets must reference an
                    // existing account id.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_deployment_postures_scope_account")
                            .from(DeploymentPostures::Table, DeploymentPostures::ScopeId)
                            .to(Accounts::Table, Accounts::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DeploymentPostures::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum DeploymentPostures {
    Table,
    ScopeKind,
    ScopeId,
    Posture,
    ChangedBy,
    ChangedAt,
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    Id,
}
