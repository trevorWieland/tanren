//! Add `approval_policies` table for organization approval gates.

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
        create_approval_policies_table(manager).await?;
        create_approval_policies_unique_index(manager).await?;
        create_approval_policies_org_index(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_approval_policies_org_index(manager).await?;
        drop_approval_policies_unique_index(manager).await?;
        drop_approval_policies_table(manager).await
    }
}

async fn create_approval_policies_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mut fk_org = ForeignKey::create();
    fk_org
        .name("fk_approval_policies_org")
        .from(ApprovalPolicies::Table, ApprovalPolicies::OrgId)
        .to(Organizations::Table, Organizations::Id)
        .on_delete(ForeignKeyAction::Cascade)
        .on_update(ForeignKeyAction::Cascade);
    manager
        .create_table(
            Table::create()
                .table(ApprovalPolicies::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ApprovalPolicies::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(ApprovalPolicies::OrgId).uuid().not_null())
                .col(
                    ColumnDef::new(ApprovalPolicies::GatedAction)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApprovalPolicies::RequiredApprovals)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApprovalPolicies::PermittedApproverPermission)
                        .string()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApprovalPolicies::Version)
                        .small_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApprovalPolicies::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ApprovalPolicies::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .foreign_key(&mut fk_org)
                .to_owned(),
        )
        .await?;
    // Add CHECK constraint via a separate DDL statement. SQLite does not
    // support ALTER TABLE ADD CONSTRAINT, so we use an unprepared
    // statement only for Postgres. The application-level validation
    // (required_approvals_to_i16 rejects zero) is the primary guard;
    // the DB-level CHECK is defense-in-depth for the production
    // Postgres deployment.
    add_required_approvals_check(manager).await
}

/// CHECK constraint: `required_approvals > 0`. A policy requiring zero
/// approvals would silently bypass the approval gate — this is a
/// security-sensitive invariant enforced at the database level for the
/// production Postgres deployment. `SQLite` does not support ALTER TABLE
/// ADD CONSTRAINT, so the check is skipped there; application-level
/// validation (`required_approvals_to_i16` rejects zero) is the
/// primary guard in all cases.
async fn add_required_approvals_check(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if matches!(
        manager.get_database_backend(),
        sea_orm::DatabaseBackend::Postgres
    ) {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE approval_policies ADD CONSTRAINT \
                 chk_approval_policies_required_approvals_positive \
                 CHECK (required_approvals > 0)",
            )
            .await?;
    }
    Ok(())
}

/// Unique index on `(org_id, gated_action)` so each action has at most
/// one rule per organization. This index also covers
/// `find_required_approval_for_action` as a single keyed read.
async fn create_approval_policies_unique_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_approval_policies_org_action_unique")
                .table(ApprovalPolicies::Table)
                .col(ApprovalPolicies::OrgId)
                .col(ApprovalPolicies::GatedAction)
                .unique()
                .to_owned(),
        )
        .await
}

/// Secondary index on `org_id` for `list_approval_policies` scans.
async fn create_approval_policies_org_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name("idx_approval_policies_org_id")
                .table(ApprovalPolicies::Table)
                .col(ApprovalPolicies::OrgId)
                .to_owned(),
        )
        .await
}

async fn drop_approval_policies_org_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_approval_policies_org_id")
                .table(ApprovalPolicies::Table)
                .to_owned(),
        )
        .await
}

async fn drop_approval_policies_unique_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name("idx_approval_policies_org_action_unique")
                .table(ApprovalPolicies::Table)
                .to_owned(),
        )
        .await
}

async fn drop_approval_policies_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(Table::drop().table(ApprovalPolicies::Table).to_owned())
        .await
}

#[derive(DeriveIden)]
enum ApprovalPolicies {
    Table,
    Id,
    OrgId,
    GatedAction,
    RequiredApprovals,
    PermittedApproverPermission,
    Version,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}
