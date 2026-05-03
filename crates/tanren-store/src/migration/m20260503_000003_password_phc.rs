//! R-0001 sub-PR 5 migration: replace the legacy `password_hash` /
//! `password_salt` byte-vec pair with a single `password_phc` TEXT
//! column carrying the Argon2id PHC string.
//!
//! R-0001 is the first feature to ship; there are no historical accounts
//! whose hashes need preserving. The migration adds the new column with
//! an empty default, then drops the two legacy columns. Any rows that
//! happen to exist (test fixtures only) lose their hashes — that is the
//! intended behaviour: the legacy SHA-256 hashes are not portable to
//! Argon2id and should not be retained.
//!
//! The `down` migration restores the legacy column shape with empty
//! defaults; PHC data is unrecoverable on rollback.

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
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .add_column(
                        ColumnDef::new(Accounts::PasswordPhc)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .drop_column(Accounts::PasswordHash)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .drop_column(Accounts::PasswordSalt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .add_column(
                        ColumnDef::new(Accounts::PasswordSalt)
                            .binary()
                            .not_null()
                            .default(Vec::<u8>::new()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .add_column(
                        ColumnDef::new(Accounts::PasswordHash)
                            .binary()
                            .not_null()
                            .default(Vec::<u8>::new()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Accounts::Table)
                    .drop_column(Accounts::PasswordPhc)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Accounts {
    Table,
    PasswordHash,
    PasswordSalt,
    PasswordPhc,
}
