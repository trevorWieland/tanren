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

        manager
            .get_connection()
            .execute_unprepared(
                r"
                INSERT INTO organizations (id, name, name_normalized, created_at)
                SELECT m.org_id,
                       'Migrated Org ' || CAST(m.org_id AS TEXT),
                       'migrated-org-' || CAST(m.org_id AS TEXT),
                       MIN(m.created_at)
                FROM memberships m
                WHERE NOT EXISTS (
                    SELECT 1 FROM organizations o WHERE o.id = m.org_id
                )
                GROUP BY m.org_id
                ",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r"
                INSERT INTO organizations (id, name, name_normalized, created_at)
                SELECT i.inviting_org_id,
                       'Migrated Org ' || CAST(i.inviting_org_id AS TEXT),
                       'migrated-org-' || CAST(i.inviting_org_id AS TEXT),
                       now()
                FROM (SELECT DISTINCT inviting_org_id FROM invitations) AS i
                WHERE NOT EXISTS (
                    SELECT 1 FROM organizations o WHERE o.id = i.inviting_org_id
                )
                ",
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
