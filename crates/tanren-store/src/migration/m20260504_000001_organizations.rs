//! R-0002 migration: create the `organizations` table and add a
//! `permissions` bigint column to `memberships`.

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
                    .table(Organizations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Organizations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Organizations::Name).text().not_null())
                    .col(
                        ColumnDef::new(Organizations::NameNormalized)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Organizations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Memberships::Table)
                    .add_column(
                        ColumnDef::new(Memberships::Permissions)
                            .big_integer()
                            .not_null()
                            .default(0i64),
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
            .drop_table(Table::drop().table(Organizations::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
    Name,
    NameNormalized,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Memberships {
    Table,
    Permissions,
}
