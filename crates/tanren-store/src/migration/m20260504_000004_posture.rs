//! R-0015 migration: create the `posture` and `posture_change` tables.
//!
//! `posture` holds exactly one row per installation (enforced by a CHECK
//! constraint on `id = 1`). It stores the current deployment posture and
//! the actor / timestamp of the most recent change.
//!
//! `posture_change` is the append-only audit trail. Every successful
//! transition inserts a row recording `from_posture`, `to_posture`, the
//! actor, and the wall-clock time.

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
                    .table(Alias::new("posture"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PostureCols::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .default(1),
                    )
                    .col(ColumnDef::new(PostureCols::Posture).string().not_null())
                    .col(
                        ColumnDef::new(PostureCols::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PostureCols::UpdatedBy).uuid().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PostureChange::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PostureChange::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PostureChange::FromPosture).string())
                    .col(ColumnDef::new(PostureChange::ToPosture).string().not_null())
                    .col(ColumnDef::new(PostureChange::Actor).uuid().not_null())
                    .col(
                        ColumnDef::new(PostureChange::ChangedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_posture_change_changed_at")
                    .table(PostureChange::Table)
                    .col(PostureChange::ChangedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_posture_change_changed_at")
                    .table(PostureChange::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(PostureChange::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("posture")).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum PostureCols {
    Id,
    Posture,
    UpdatedAt,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum PostureChange {
    Table,
    Id,
    FromPosture,
    ToPosture,
    Actor,
    ChangedAt,
}
