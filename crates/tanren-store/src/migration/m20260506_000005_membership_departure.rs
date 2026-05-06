//! R-0007 migration: create the `member_in_flight_work` placeholder
//! table used by departure-flow BDD fixtures.

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
        manager
            .create_table(
                Table::create()
                    .table(MemberInFlightWork::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemberInFlightWork::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MemberInFlightWork::AccountId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemberInFlightWork::OrgId).uuid().not_null())
                    .col(
                        ColumnDef::new(MemberInFlightWork::Description)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemberInFlightWork::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_member_in_flight_work_account_org")
                    .table(MemberInFlightWork::Table)
                    .col(MemberInFlightWork::AccountId)
                    .col(MemberInFlightWork::OrgId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_member_in_flight_work_account_org")
                    .table(MemberInFlightWork::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(MemberInFlightWork::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum MemberInFlightWork {
    Table,
    Id,
    AccountId,
    OrgId,
    Description,
    CreatedAt,
}
